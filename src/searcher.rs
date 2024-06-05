use crate::misc::Image;

use std::path::PathBuf;
use std::thread;
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, Ordering};
use std::collections::{HashMap, HashSet};
use std::thread::JoinHandle;

use walkdir::WalkDir;

use magnetic::spmc::spmc_queue;
use magnetic::buffer::dynamic::DynamicBuffer;
use magnetic::{Producer, Consumer};

use image_hasher::HasherConfig;

use maplit::hashset;

use lazy_static::lazy_static;

lazy_static! {
    pub static ref SUPPORTED_EXTS: HashSet<&'static str> = hashset!{
        "jpg",
        "jpeg",
        "avif",
        "avif",
        "bmp",
        "dds",
        "exr",
        "gif",
        "hdr",
        "ico",
        "png",
        "pnm",
        "qoi",
        "tga",
        "tif",
        "tiff",
        "webp",
    };
}

pub struct SearchResults {
    pub duplicates: Vec<Vec<Image>>,
    pub errors: Vec<String>,
}

impl SearchResults {
    fn empty() -> SearchResults {
        SearchResults {
            duplicates: vec![],
            errors: vec![],
        }
    }
}

struct SearcherInner {
    root: PathBuf,
    follow_sym: bool,
    num_threads: usize,
    max_depth: Option<usize>,
    exts: HashSet<String>, // Extentions to consider
    cancel: AtomicBool,
}

impl SearcherInner {
    fn search(&self) -> SearchResults {
        // The size is somewhat arbitrary, but its expected to take longer to
        // process each unit of work than to produce it, so having twice as large
        // a buffer as there are worker threads should ensure the workers always
        // have work waiting.
        let work_buf = DynamicBuffer::new(2 * self.num_threads)
            .expect("Error allocationg spmc buffer");
        let (queue_p, queue_c) = spmc_queue::<Option<std::path::PathBuf>, _>(work_buf);
        let queue_c = Arc::new(queue_c);

        let map = Arc::new(Mutex::new(HashMap::new()));
        let errors = Arc::new(Mutex::new(vec![]));

        let mut threads = Vec::new();
        for _ in 1..=self.num_threads {
            let queue_c = queue_c.clone();
            let map = map.clone();
            let errors = errors.clone();

            threads.push(thread::spawn(move || {
                let hasher = HasherConfig::new().to_hasher();
                loop {
                    let path = match queue_c.pop().expect("queue pop error") {
                        Some(x) => x,
                        None => return, // We're done!
                    };
                    let image = match image::open(path.clone()) {
                        Ok(x) => x,
                        Err(e) => {
                            let err = format!("Error opening image {}: {e}", path.display());
                            errors.lock().expect("error vec lock error").push(err);
                            continue;
                        },
                    };

                    let hash = hasher.hash_image(&image);
                    let mut map = map.lock().expect("image hash hashmap lock error");
                    let v = map.entry(hash).or_insert(Vec::new());
                    v.push(path);
                }
            }));
        }

        let mut walker = WalkDir::new(self.root.clone()).follow_links(self.follow_sym);
        if let Some(d) = self.max_depth { walker = walker.max_depth(d); }
        for entry in walker {
            let entry = match entry {
                Ok(x) => x,
                Err(e) => {
                    let err = format!("Error walking directory: {e}");
                    errors.lock().expect("error vec lock error").push(err);
                    continue;
                },
            };
            if entry.file_type().is_dir() { continue; }

            let path = entry.path();
            if let Some(ext) = path.extension() {
                let s = ext.to_string_lossy();
                if !self.exts.contains(&*s) { continue; }
                queue_p.push(Some(path.to_owned())).expect("queue push error");
            }

            if self.cancel.load(Ordering::Relaxed) {
                break;
            }
        }

        for _ in 1..=threads.len() {
            queue_p.push(None).expect("queue push error");
        }

        for t in threads {
            t.join().expect("thread join error");
        }

        if self.cancel.load(Ordering::Relaxed) {
            return SearchResults::empty();
        }

        let mut errors = Arc::into_inner(errors).expect("arc into_inner error")
            .into_inner().expect("mutex into_inner error");

        let map = Arc::into_inner(map).expect("arc into_inner error")
            .into_inner().expect("mutex into_inner");

        let mut duplicates = vec![];
        for (_, dups) in map.into_iter() {
            if dups.len() <= 1 {
                continue;
            }

            let mut v = vec![];
            for path in dups {
                match Image::load(path) {
                    Ok(x) => v.push(x),
                    Err(e) => errors.push(e),
                };

                if self.cancel.load(Ordering::Relaxed) {
                    return SearchResults::empty();
                }
            }
            duplicates.push(v);
        }

        SearchResults {
            duplicates,
            errors,
        }
    }

}

pub struct Searcher {
    inner: Arc<SearcherInner>,
    thread: Option<JoinHandle<SearchResults>>,
}

impl Searcher {
    pub fn new(
        root: PathBuf,
        follow_sym: bool,
        num_threads: usize,
        max_depth: Option<usize>,
        exts: HashSet<String>
    ) -> Searcher {
        Searcher {
            inner: Arc::new(SearcherInner{
                root,
                follow_sym,
                num_threads,
                max_depth,
                exts,
                cancel: AtomicBool::new(false),
            }),
            thread: None,
        }
    }
    
    pub fn cancel(&self) {
        self.inner.cancel.store(true, Ordering::Relaxed);
    }

    // TODO: if both this and cancel() are relaxed, is a single thread that calls
    // cancel() then was_canceled() be guarenteed to see was_canceled() return true?
    pub fn was_canceled(&self) -> bool {
        self.inner.cancel.load(Ordering::Relaxed)
    }

    pub fn launch_search(&mut self) {
        self.inner.cancel.store(false, Ordering::Relaxed);
        let inner = self.inner.clone();
        self.thread = Some(thread::spawn(move || {
            inner.search()
        }));
    }
    
    // Panics if not launch_search was never called or hasn't been called since
    // the previous join()
    pub fn is_finished(&self) -> bool {
        self.thread.as_ref()
            .expect("thread missing (was search_async() called?)")
            .is_finished()
    }
    
    // Panics if not search_async was never called
    // Panics on thread join errors
    pub fn join(&mut self) -> SearchResults {
        self.thread.take()
            .expect("thread missing (was search_async() called?)")
            .join()
            .expect("thread join error")

    }
}



