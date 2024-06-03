
use crate::{Phase, Action, Modal};
use crate::searching_phase::SearchingPhase;
use crate::searcher::Searcher;

use std::path::PathBuf;

use eframe::egui;

// User options
#[derive(Default)]
pub struct UserOpts {
    pub root: PathBuf,
    pub follow_sym: bool,
    pub limit_depth: bool,
    pub max_depth: String,
    pub num_worker_threads: String,
}

pub struct StartupPhase {
    opts: UserOpts,
}

impl StartupPhase {
    pub fn new(root: PathBuf) -> StartupPhase {
        StartupPhase {
            opts: UserOpts {
                root,
                follow_sym: false,
                limit_depth: false,
                max_depth: "".to_string(),
                num_worker_threads: num_cpus::get().to_string(),
            },
        }
    }

    pub fn with_opts(opts: UserOpts) -> StartupPhase {
        StartupPhase{opts}
    }

    fn make_searching_phase(&mut self) -> Action {
        let mut max_depth = None;
        if self.opts.limit_depth {
            max_depth = match self.opts.max_depth.parse::<usize>() {
                Ok(x) => Some(x),
                Err(e) => {
                    return Action::Modal(Modal::new(
                        "Error parsing depth limit".to_string(),
                        e.to_string(),
                    ))
                },
            };
            if max_depth == Some(0usize) {
                return Action::Modal(Modal::new(
                    "Invalid depth limit".to_string(),
                    "A depth limit of 0 doesn't search at all".to_string(),
                ));
            }
        }

        let num_worker_threads = match self.opts.num_worker_threads.parse::<usize>() {
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
            self.opts.root.clone(),
            self.opts.follow_sym,
            num_worker_threads,
            max_depth,
        );
        searcher.launch_search();
        let opts = std::mem::replace(&mut self.opts, UserOpts::default());
        Action::Trans(Box::new(SearchingPhase::new(opts, searcher)))
    }
}

impl Phase for StartupPhase {
    fn render(&mut self, _ctx: &egui::Context, ui: &mut egui::Ui) -> Action {
        ui.horizontal(|ui| {
            ui.strong("Root: ".to_string());
            ui.monospace(self.opts.root.display().to_string());
        });

        if ui.button("Choose...").clicked() {
            if let Some(path) = rfd::FileDialog::new().pick_folder() {
                self.opts.root = path;
            }
        }

        ui.separator();

        ui.collapsing("Advanced", |ui| {
            ui.checkbox(&mut self.opts.follow_sym, "Follow Symlinks");
            ui.horizontal(|ui| {
                ui.checkbox(&mut self.opts.limit_depth, "Depth Limit");
                let depth_field = egui::TextEdit::singleline(&mut self.opts.max_depth);
                ui.add_enabled(self.opts.limit_depth, depth_field);
            });

            ui.horizontal(|ui| {
                ui.label("Num Worker Threads");
                ui.text_edit_singleline(&mut self.opts.num_worker_threads);
            });
        });

        ui.separator();

        if ui.button("Search").clicked() {
            return self.make_searching_phase();
        }

        Action::None
    }
}

