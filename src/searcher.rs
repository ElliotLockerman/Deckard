use crate::misc::Image;

use std::path::PathBuf;
use std::thread;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::collections::HashSet;
use std::thread::JoinHandle;

use walkdir::WalkDir;

use image_hasher::HasherConfig;

use maplit::hashset;

use lazy_static::lazy_static;

use rayon::prelude::*;

use dashmap::{DashMap, DashSet};


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
    max_depth: Option<usize>,
    exts: HashSet<String>, // Extentions to consider
    cancel: AtomicBool,
}

impl SearcherInner {

    fn search(&self) -> SearchResults {
        let map = DashMap::new();
        let errors = DashSet::new();

        let hasher = HasherConfig::new().to_hasher();
        let mut walker = WalkDir::new(self.root.clone()).follow_links(self.follow_sym);
        if let Some(d) = self.max_depth { walker = walker.max_depth(d); }
        let _: Result<(), ()> = walker.into_iter().par_bridge().map(|entry| {

            if self.cancel.load(Ordering::Relaxed) {
                return Err(());
            }

            let entry = match entry {
                Ok(x) => x,
                Err(e) => {
                    let err = format!("Error walking directory: {e}");
                    errors.insert(err);
                    return Ok(());
                },
            };
            if entry.file_type().is_dir() {
                return Ok(());
            }

            let path = entry.path();
            let Some(ext) = path.extension() else {
                return Ok(());
            };
            let s = ext.to_string_lossy();
            if !self.exts.contains(&*s) {
                return Ok(());
            }

            // I have seen image::open() panic on (presumably) malformed files.
            let image = match std::panic::catch_unwind(|| image::open(path)) {
                Ok(Ok(x)) => x,
                err => { 
                    let msg = match err {
                        Err(_) => format!("Panic opening image {}", path.display()),
                        Ok(Err(e)) => format!("Error opening image {}: {e}", path.display()),
                        Ok(Ok(_)) => unreachable!(),
                    };
                    errors.insert(msg);
                    return Ok(())
                },
            };

            let hash = hasher.hash_image(&image);
            map.entry(hash).or_insert(DashSet::new()).insert(path.to_path_buf());

            Ok(())
        }).collect();


        // This part doesn't take very long (essentially 0 benefit for 
        // paralleization), and requires a lot of extra complexity to make it 
        // cancelable with rayon considering the nested loops.
        let mut duplicates = vec![];
        for (_, dups) in map.into_iter() {
            if self.cancel.load(Ordering::Relaxed) {
                return SearchResults::empty();
            }

            if dups.len() <= 1 {
                continue;
            }

            let mut v = vec![];
            for path in dups {
                match Image::load(path) {
                    Ok(x) => v.push(x),
                    Err(e) => { errors.insert(e); },
                }

                if self.cancel.load(Ordering::Relaxed) {
                    return SearchResults::empty();
                }
            }
            duplicates.push(v);
        }

        SearchResults {
            duplicates,
            errors: errors.into_iter().collect(),
        }
    }
}

pub type PhantomUnsync = std::marker::PhantomData<std::cell::Cell<()>>;
pub type PhantomUnsend = std::marker::PhantomData<std::sync::MutexGuard<'static, ()>>;

// Designed for (and only tested for) a single run, but in principle could be
// used again after wait_for_search(). Not Sync or Send because of the Relaxed
// accesses to store - if you call cancel(), then another thread calls
// was_canceled(), it may get false, even if it sees other data up-to-date. This
// doesn't matter for the worker threads because the check repeatedly, and
// canceling is eventual.
pub struct Searcher {
    inner: Arc<SearcherInner>,
    thread: Option<JoinHandle<SearchResults>>,
    unsync: PhantomUnsync,
    unsend: PhantomUnsend,
}

impl Searcher {
    pub fn new(
        root: PathBuf,
        follow_sym: bool,
        max_depth: Option<usize>,
        exts: HashSet<String>
    ) -> Searcher {
        Searcher {
            inner: Arc::new(SearcherInner{
                root,
                follow_sym,
                max_depth,
                exts,
                cancel: AtomicBool::new(false),
            }),
            thread: None,
            unsync: Default::default(),
            unsend: Default::default(),
        }
    }
    
    pub fn cancel(&self) {
        self.inner.cancel.store(true, Ordering::Relaxed);
    }

    pub fn was_canceled(&self) -> bool {
        self.inner.cancel.load(Ordering::Relaxed)
    }

    pub fn launch_search(&mut self) {
        assert!(
            !self.thread.is_some(),
            "launch_search() called twice without wait_for_search() between"
        );
        self.inner.cancel.store(false, Ordering::Relaxed);
        let inner = self.inner.clone();
        self.thread = Some(thread::spawn(move || {
            inner.search()
        }));
    }
    
    // Panics if not launch_search was never called or hasn't been called (or if
    // join() was already called)
    pub fn is_finished(&self) -> bool {
        self.thread.as_ref()
            .expect("thread missing")
            .is_finished()
    }
    
    // Panics if not search_async was never called (or if join() was already
    // called)
    // Panics on thread join errors
    pub fn wait_for_search(&mut self) -> SearchResults {
        self.thread.take()
            .expect("thread missing")
            .join()
            .expect("thread join error")

    }
}



