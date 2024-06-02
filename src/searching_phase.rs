
use crate::{Phase, Action, Image};
use crate::startup_phase::StartupPhase;
use crate::output_phase::OutputPhase;
use crate::search::Searcher;

use std::path::PathBuf;
use std::io::Read;


use eframe::egui;

pub struct SearchingPhase {
    root: PathBuf,
    searcher: Searcher,
}

impl SearchingPhase {
    pub fn new(root: PathBuf, searcher: Searcher) -> SearchingPhase {
        SearchingPhase {
            root,
            searcher,
        }
    }

    fn make_output_phase(&mut self) -> Box<dyn Phase> {
        assert!(self.searcher.is_finished());
        let results = self.searcher.join();

        let mut errors = results.errors;
        let paths = results.duplicates;

        let mut images = vec![];
        // TODO: do this in a thread (it doesn't seem to be a problem in practice)?
        for dups in &paths {
            let mut vec = vec![];
            for path in dups {
                // Manually loading the image and passing it as bytes is the only way I could get it to handle URIs with spaces
                let mut buffer = vec![];
                let mut file = match std::fs::File::open(path.clone()) {
                    Ok(x) => x,
                    Err(e) => {
                        errors.push(format!("Error opening {}: {}", path.display(), e.to_string()));
                        continue;
                    }
                };
                if let Err(e) = file.read_to_end(&mut buffer) {
                    errors.push(format!("Error reading {}: {}", path.display(), e.to_string()));
                    continue;

                }
                let file_size = buffer.len();

                let dimm = image::load_from_memory(&buffer).ok().map(|img| {
                    (img.width(), img.height())
                });

                vec.push(Image{
                    path: path.clone(),
                    handle: format!("{}", path.display()),
                    buffer: egui::load::Bytes::from(buffer),
                    file_size,
                    dimm,
                });
            }
            images.push(vec);
        }

        let root = std::mem::replace(&mut self.root, PathBuf::new());
        Box::new(OutputPhase::new(root, images, errors))
    }
}

impl Phase for SearchingPhase {

    fn render(&mut self, _ctx: &egui::Context, ui: &mut egui::Ui) -> Action {
        if self.searcher.is_finished() {
            if self.searcher.was_canceled() {
                self.searcher.join();
                let root = std::mem::replace(&mut self.root, PathBuf::new());
                return Action::Trans(Box::new(StartupPhase::new(root)));
            } else {
                return Action::Trans(self.make_output_phase());
            }
        }

        if ui.button("<- New Search").clicked() {
            self.searcher.cancel();
        }

        ui.separator();


        ui.horizontal(|ui| {
            ui.strong(format!("Searching"));
            ui.monospace(format!("{}", self.root.display()));
        });

        ui.centered_and_justified(|ui| {
            let spinner = egui::widgets::Spinner::new().size(256.0);
            ui.add(spinner);
        });

        Action::None
    }
}

