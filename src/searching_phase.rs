
use crate::{Phase, Action};
use crate::startup_phase::{StartupPhase, UserOpts};
use crate::output_phase::OutputPhase;
use crate::searcher::Searcher;

use eframe::egui;

pub struct SearchingPhase {
    opts: UserOpts,
    searcher: Searcher,
}

impl SearchingPhase {

    // Eyeballed, seems good for a reasonable variety of window sizes
    const SPINNER_SIZE: f32 = 256.0;

    pub fn new(opts: UserOpts, searcher: Searcher) -> SearchingPhase {
        SearchingPhase {
            opts,
            searcher,
        }
    }

    fn make_output_phase(&mut self) -> Box<dyn Phase> {
        assert!(self.searcher.is_finished());
        let results = self.searcher.join();
        let opts = std::mem::take(&mut self.opts);
        Box::new(OutputPhase::new(opts, results.duplicates, results.errors))
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
            let spinner = egui::widgets::Spinner::new().size(Self::SPINNER_SIZE);
            ui.add(spinner);
        });

        Action::None
    }
}

