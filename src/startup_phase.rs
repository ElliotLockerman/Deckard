
use crate::{Phase, Action, Modal};
use crate::searching_phase::SearchingPhase;
use crate::search::Searcher;

use std::path::PathBuf;

use eframe::egui;

pub struct StartupPhase {
    root: PathBuf,
    follow_sym: bool,
    limit_depth: bool,
    max_depth: String,
    num_worker_threads: String,
}

impl StartupPhase {
    pub fn new(root: PathBuf) -> StartupPhase {
        StartupPhase {
            root,
            follow_sym: true,
            limit_depth: false,
            max_depth: "".to_string(),
            num_worker_threads: num_cpus::get().to_string(),
        }
    }

    fn make_searching_phase(&mut self) -> Action {
        let mut max_depth = None;
        if self.limit_depth {
            match self.max_depth.parse::<usize>() {
                Ok(x) => max_depth = Some(x),
                Err(e) => {
                    return Action::Modal(Modal::new(
                        "Error parsing depth limit".to_string(),
                        e.to_string(),
                    ))
                },
            }
            if max_depth == Some(0usize) {
                return Action::Modal(Modal::new(
                    "Invalid depth limit".to_string(),
                    "A depth limit of 0 doesn't search at all".to_string(),
                ));
            }
        }

        let num_worker_threads = match self.num_worker_threads.parse::<usize>() {
            Ok(x) => x,
            Err(e) => {
                return Action::Modal(Modal::new(
                    "Error parsing num worker threads".to_string(),
                    e.to_string(),
                ));
            },
        };
        if num_worker_threads == 0 {
            return Action::Modal(Modal::new(
                "Invalid num worker threads".to_string(),
                "At least 1 worker thread is required".to_string(),
            ));
        }

        let mut searcher = Searcher::new(
            self.root.clone(),
            self.follow_sym,
            num_worker_threads,
            max_depth,
        );
        searcher.launch_search();
        let root = std::mem::replace(&mut self.root, PathBuf::new());
        Action::Trans(Box::new(SearchingPhase::new(root, searcher)))
    }
}

impl Phase for StartupPhase {
    fn render(&mut self, _ctx: &egui::Context, ui: &mut egui::Ui) -> Action {
        ui.horizontal(|ui| {
            ui.strong("Root: ".to_string());
            ui.monospace(format!("{}", self.root.display()));
        });

        if ui.button("Choose...").clicked() {
            if let Some(path) = rfd::FileDialog::new().pick_folder() {
                self.root = path;
            }
        }

        ui.separator();

        ui.collapsing("Advanced", |ui| {
            ui.checkbox(&mut self.follow_sym, "Follow Symlinks");
            ui.horizontal(|ui| {
                ui.checkbox(&mut self.limit_depth, "Depth Limit");
                let depth_field = egui::TextEdit::singleline(&mut self.max_depth);
                ui.add_enabled(self.limit_depth, depth_field);
            });

            ui.horizontal(|ui| {
                ui.label("Num Worker Threads");
                ui.text_edit_singleline(&mut self.num_worker_threads);
            });
        });

        ui.separator();

        if ui.button("Search").clicked() {
            return self.make_searching_phase();
        }

        Action::None
    }
}

