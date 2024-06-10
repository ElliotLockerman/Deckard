
use crate::ROOT_KEY;

use crate::{Phase, DynPhase, Result};
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

    pub fn into_dyn(self) -> DynPhase {
        Box::new(self)
    }

    fn make_output_phase(&mut self) -> DynPhase {
        assert!(self.searcher.is_finished());
        let results = self.searcher.join();
        OutputPhase::new(self.opts.take(), results.duplicates, results.errors).into_dyn()
    }
}

impl Phase for SearchingPhase {
    fn render(&mut self, _ctx: &egui::Context, ui: &mut egui::Ui) -> Result<Option<DynPhase>> {
        if self.searcher.is_finished() {
            assert!(!self.searcher.was_canceled());
            return Ok(Some(self.make_output_phase()));
        }

        let resp = ui.horizontal(|ui| {
            if ui.button("<- New Search").clicked() 
                || ui.input(|i| i.key_pressed(egui::Key::Escape)) {

                self.searcher.cancel();
                return Some(StartupPhase::new_with_opts(self.opts.take()).into_dyn());
            }

            ui.horizontal(|ui| {
                ui.strong("Searching");
                ui.monospace(self.opts.root.display().to_string());
            });

            None
        });

        if resp.inner.is_some() {
            return Ok(resp.inner);
        }

        ui.separator();

        ui.centered_and_justified(|ui| {
            let spinner = egui::widgets::Spinner::new().size(Self::SPINNER_SIZE);
            ui.add(spinner);
        });

        Ok(None)
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        storage.set_string(ROOT_KEY, self.opts.root.to_string_lossy().into());
    }
}

