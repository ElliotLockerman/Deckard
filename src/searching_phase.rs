
use crate::{Phase, Action};
use crate::startup_phase::{StartupPhase, UserOpts};
use crate::output_phase::OutputPhase;
use crate::searcher::Searcher;
use crate::misc::Image;

use eframe::egui;

// Eyeballed, seems good for a reasonable variety of window sizes
const SPINNER_SIZE: f32 = 256.0;

pub struct SearchingPhase {
    opts: UserOpts,
    searcher: Searcher,
}

impl SearchingPhase {
    pub fn new(opts: UserOpts, searcher: Searcher) -> SearchingPhase {
        SearchingPhase {
            opts,
            searcher,
        }
    }

    fn make_output_phase(&mut self) -> Box<dyn Phase> {
        assert!(self.searcher.is_finished());
        let results = self.searcher.join();

        let mut errors = results.errors;
        let paths = results.duplicates;

        let mut images = vec![];
        // TODO: do this in a thread? (it doesn't seem to be a problem in practice)
        for dups in paths {
            let mut vec = vec![];
            for path in dups {
                let image = match Image::load(path) {
                    Ok(x) => x,
                    Err(e) => {
                        errors.push(e);
                        continue;
                    },
                };
                vec.push(image);
            }
            images.push(vec);
        }

        let opts = std::mem::take(&mut self.opts);
        Box::new(OutputPhase::new(opts, images, errors))
    }
}

impl Phase for SearchingPhase {
    fn render(&mut self, _ctx: &egui::Context, ui: &mut egui::Ui) -> Action {
        if self.searcher.is_finished() {
            if self.searcher.was_canceled() {
                self.searcher.join();
                let opts = std::mem::take(&mut self.opts);
                return Action::Trans(Box::new(StartupPhase::new_with_opts(opts)));
            } else {
                return Action::Trans(self.make_output_phase());
            }
        }

        let resp = ui.horizontal(|ui| {
            if ui.button("<- New Search").clicked() 
                || ui.input(|i| i.key_pressed(egui::Key::Escape)) {

                self.searcher.cancel();
                let opts = std::mem::take(&mut self.opts);
                return Some(Action::Trans(Box::new(StartupPhase::new_with_opts(opts))));
            }

            ui.horizontal(|ui| {
                ui.strong("Searching");
                ui.monospace(self.opts.root.display().to_string());
            });

            None
        });

        if let Some(action) = resp.inner {
            return action;
        }

        ui.separator();

        ui.centered_and_justified(|ui| {
            let spinner = egui::widgets::Spinner::new().size(SPINNER_SIZE);
            ui.add(spinner);
        });

        Action::None
    }
}

