use std::path::PathBuf;
use std::thread;
use std::sync::{Arc, Mutex};
use std::collections::HashMap;

use walkdir::WalkDir;

use magnetic::spmc::spmc_queue;
use magnetic::buffer::dynamic::DynamicBuffer;
use magnetic::{Producer, Consumer};

use image_hasher::HasherConfig;

use maplit::hashset;

pub struct SearchResults {
    pub duplicates: Vec<Vec<PathBuf>>,
    pub errors: Vec<String>,
}

pub fn search(
    root: PathBuf,
    follow_sym: bool,
    max_depth: Option<usize>,
    num_threads: Option<usize>) -> SearchResults {

    let (p1, c1) = spmc_queue::<
            Option<std::path::PathBuf>,
            DynamicBuffer<Option<std::path::PathBuf>>
        >(DynamicBuffer::new(32).unwrap());

    let c1 = Arc::new(c1);

    let map = Arc::new(Mutex::new(HashMap::new()));
    let errors = Arc::new(Mutex::new(vec![]));

    let num_threads = num_threads.unwrap_or_else(|| num_cpus::get());

    let mut threads = Vec::new();
    for _ in 1..=num_threads {
        let c1 = c1.clone();
        let map = map.clone();
        let errors = errors.clone();

        threads.push(thread::spawn(move || {
            let hasher = HasherConfig::new().to_hasher();
            while let Ok(v) = c1.pop() {
                if let Some(path) = v {
                    let image = image::open(path.clone());
                    if let Err(e) = image {
                        let err = format!("Error opening image {}: {}", path.display(), e);
                        errors.lock().unwrap().push(err);
                        continue;
                    }

                    let img = image.unwrap();
                    let hash = hasher.hash_image(&img);

                    let mut map = map.lock().unwrap();
                    let v = map.entry(hash).or_insert(Vec::new());
                    v.push(path);
                } else {
                    return;
                }

            }
            unreachable!();
        }));
    }

    let exts = hashset!{"jpg", "jpeg", "avif", "bmp", "dds", "exr", "gif", "hdr", "ico", "png", "pnm", "qoi", "tga", "tiff", "webp"};
    let mut walker = WalkDir::new(root).follow_links(follow_sym);
    if let Some(d) = max_depth { walker = walker.max_depth(d); }
    for entry in walker {
        if let Err(e) = entry {
            let err = format!("Error walking directory: {}", e);
            errors.lock().unwrap().push(err);
            continue;
        }


        let entry = entry.unwrap();
        if entry.file_type().is_dir() { continue; }

        let path = entry.path();
        if let Some(ext) = path.extension() {
            let s = ext.to_string_lossy();
            if !exts.contains(&*s) { continue; }
            p1.push(Some(path.to_owned())).unwrap();
        }
    }

    for _ in 1..=threads.len() {
        p1.push(None).unwrap();
    }

    for t in threads {
        t.join().unwrap();
    }

    let map = Arc::into_inner(map).unwrap().into_inner().unwrap();
    let duplicates = map.into_iter()
        .filter_map(|(_, x)| if x.len() > 1 { Some(x) } else { None })
        .collect();

    SearchResults {
        duplicates,
        errors: Arc::into_inner(errors).unwrap().into_inner().unwrap(),
    }
}



