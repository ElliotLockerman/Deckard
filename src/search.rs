use std::path::PathBuf;
use std::thread;
use std::sync::{Arc, Mutex};
use std::collections::HashMap;

use walkdir::WalkDir;

use magnetic::spmc::spmc_queue;
use magnetic::buffer::dynamic::DynamicBuffer;
use magnetic::{Producer, Consumer};

use img_hash::{ImageHash, HashType};

use maplit::hashset;


pub fn search(
    root: PathBuf,
    follow_sym: bool,
    max_depth: Option<usize>,
    num_threads: Option<usize>) -> Vec<Vec<PathBuf>> {

    let (p1, c1) = spmc_queue::<
            Option<std::path::PathBuf>,
            DynamicBuffer<Option<std::path::PathBuf>>
        >(DynamicBuffer::new(32).unwrap());

    let c1 = Arc::new(c1);

    let map = Arc::new(Mutex::new(HashMap::new()));

    let num_threads = if let Some(x) = num_threads {
        x 
    } else {
        num_cpus::get()
    };
    let mut threads = Vec::new();
    for _ in 1..=num_threads {
        let c1 = c1.clone();
        let map = map.clone();

        threads.push(thread::spawn(move || {
            while let Ok(v) = c1.pop() {
                if let Some(path) = v {
                    let image = image::open(path.clone());
                    if let Err(e) = image {
                        println!("Error for image {}: {}", path.display(), e);
                        continue;
                    }

                    let hash = ImageHash::hash(&image.unwrap(), 8, HashType::Gradient);

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

    /*
    if !Path::new(&root).exists() {
        return Err(SearchError::RootDoesntExist);
    }
    */

    let exts = hashset!{"jpg", "jpeg", "png", "gif", "webp"};
    let mut walker = WalkDir::new(root).follow_links(follow_sym);
    if let Some(d) = max_depth { walker = walker.max_depth(d); }
    for entry in walker {
        if let Err(e) = entry {
            println!("Error: {}", e);
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

    let map = map.lock().unwrap();
    map.iter()
        .filter_map(|(_, x)| if x.len() > 1 { Some(x.clone()) } else { None })
        .collect()
}



