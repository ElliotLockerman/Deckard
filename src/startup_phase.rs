
use crate::{Phase, Action, Modal, try_act};
use crate::searching_phase::SearchingPhase;
use crate::searcher::{Searcher, SUPPORTED_EXTS};

use std::path::PathBuf;
use std::collections::HashSet;

use eframe::egui;

// User options
#[derive(Default)]
pub struct UserOpts {
    pub root: PathBuf,
    pub follow_sym: bool,
    pub limit_depth: bool,
    pub max_depth: String,
    pub num_worker_threads: String,
    pub exts: HashSet<String>,
}

pub struct StartupPhase {
    opts: UserOpts,
}

impl StartupPhase {
    pub fn new(root: PathBuf) -> StartupPhase {
        StartupPhase {
            opts: UserOpts {
                root,
                num_worker_threads: num_cpus::get().to_string(),
                ..Default::default()
            },
        }
    }

    pub fn with_opts(opts: UserOpts) -> StartupPhase {
        StartupPhase{opts}
    }

    fn parse_max_depth(&self) -> Result<Option<usize>, Action> {
        let mut max_depth = None;
        if self.opts.limit_depth {
            let depth = self.opts.max_depth.parse::<usize>().map_err(|e| {
                Action::Modal(Modal::new(
                        "Error parsing depth limit".to_string(),
                        e.to_string(),
                ))
            })?;
            if depth == 0usize {
                return Err(Action::Modal(Modal::new(
                    "Invalid depth limit".to_string(),
                    "A depth limit of 0 doesn't search at all".to_string(),
                )));
            }
            max_depth = Some(depth);
        }
        Ok(max_depth)
    }

    fn parse_num_worker_threads(&self) -> Result<usize, Action> {
        let num_worker_threads = self.opts.num_worker_threads.parse::<usize>()
            .map_err(|e| {
                Action::Modal(Modal::new(
                    "Error parsing num worker threads".to_string(),
                    e.to_string(),
                ))
            })?;

        if num_worker_threads == 0 {
            return Err(Action::Modal(Modal::new(
                "Invalid num worker threads".to_string(),
                "At least 1 worker thread is required".to_string(),
            )));
        }

        Ok(num_worker_threads)
    }

    fn make_searching_phase(&mut self) -> Action {
        let max_depth = try_act!(self.parse_max_depth());
        let num_worker_threads = try_act!(self.parse_num_worker_threads());
        let exts = SUPPORTED_EXTS.iter().map(|x| x.to_string()).collect();

        let mut searcher = Searcher::new(
            self.opts.root.clone(),
            self.opts.follow_sym,
            num_worker_threads,
            max_depth,
            exts,
        );
        searcher.launch_search();
        let opts = std::mem::take(&mut self.opts);
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

