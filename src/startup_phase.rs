
use crate::{Phase, DynPhase, Error, Result};
use crate::searching_phase::SearchingPhase;
use crate::searcher::{Searcher, SUPPORTED_EXTS};

use std::path::PathBuf;
use std::collections::HashSet;

use eframe::egui;
use egui::widgets::text_edit::TextEdit;

use itertools::Itertools;

// User options
#[derive(Default)]
pub struct UserOpts {
    pub root: PathBuf,
    pub follow_sym: bool,
    pub max_depth: String,
    pub num_worker_threads: String,
    pub exts: String,
}

impl UserOpts {
    pub fn take(&mut self) -> UserOpts {
        std::mem::take(self)
    }
}

pub struct StartupPhase {
    opts: UserOpts,
}

impl StartupPhase {
    const ROOT_KEY: &'static str = "STARTUPPHASE_ROOT";
    const NUM_WORKER_THREADS_WIDTH: f32 = 30.0;

    pub fn new_with_cc(cc: &eframe::CreationContext) -> StartupPhase {
        let root = match cc.storage {
            Some(storage) => match storage.get_string(Self::ROOT_KEY) {
                Some(path) => path.into() ,
                None =>  Self::default_root(),
            },
            None => Self::default_root(),
        };

        StartupPhase {
            opts: UserOpts {
                root,
                num_worker_threads: num_cpus::get().to_string(),
                exts: SUPPORTED_EXTS.iter().join(","),
                ..Default::default()
            },
        }
    }

    pub fn new_with_opts(opts: UserOpts) -> StartupPhase {
        StartupPhase{opts}
    }

    pub fn into_dyn(self) -> DynPhase {
        Box::new(self)
    }

    fn default_root() -> PathBuf {
        homedir::get_my_home()
            .unwrap_or_else(|_| Some(PathBuf::from("/")))
            .unwrap_or_else(|| PathBuf::from("/"))
    }

    fn parse_max_depth(&self) -> Result<Option<usize>> {
        let mut max_depth = None;
        if !self.opts.max_depth.is_empty() {
            let depth = self.opts.max_depth.parse::<usize>().map_err(|e| {
                Error::new(
                    "Error parsing depth limit".to_string(),
                    e.to_string(),
                )
            })?;
            if depth == 0usize {
                return Err(Error::new(
                    "Invalid depth limit".to_string(),
                    "A depth limit of 0 doesn't search at all".to_string(),
                ));
            }
            max_depth = Some(depth);
        }
        Ok(max_depth)
    }

    fn parse_num_worker_threads(&self) -> Result<usize, Error> {
        let num_worker_threads = self.opts.num_worker_threads.parse::<usize>()
            .map_err(|e| {
                Error::new(
                    "Error parsing num worker threads".to_string(),
                    e.to_string(),
                )
            })?;

        if num_worker_threads == 0 {
            return Err(Error::new(
                "Invalid num worker threads".to_string(),
                "At least 1 worker thread is required".to_string(),
            ));
        }

        Ok(num_worker_threads)
    }

    fn parse_exts(&self) -> Result<HashSet<String>, Error> {
        let exts: HashSet<String> = self.opts.exts
            .split(',')
            .map(|x| x.trim().to_owned())
            .filter(|x| !x.is_empty())
            .collect();

        for ext in &exts {
            if !SUPPORTED_EXTS.contains(ext.as_str()) {
                return Err(Error::new(
                    "Extension Error".to_owned(),
                    format!("Extension {ext} is not supported"),
                ));
            }
        }

        Ok(exts)
    }

    fn make_searching_phase(&mut self) -> Result<DynPhase> {
        if !self.opts.root.exists() {
            return Err(Error::new(
                "Path Error".into(),
                format!("{} doesn't exist", self.opts.root.display()),
            ));
        }

        let max_depth = self.parse_max_depth()?;
        let num_worker_threads = self.parse_num_worker_threads()?;
        let exts = self.parse_exts()?;

        let mut searcher = Searcher::new(
            self.opts.root.clone(),
            self.opts.follow_sym,
            num_worker_threads,
            max_depth,
            exts,
        );
        searcher.launch_search();
        let opts = std::mem::take(&mut self.opts);
        Ok(SearchingPhase::new(opts, searcher).into_dyn())
    }
}

impl Phase for StartupPhase {
    fn render(&mut self, _ctx: &egui::Context, ui: &mut egui::Ui) -> Result<Option<DynPhase>> {
        ui.horizontal(|ui| {
            ui.strong("Root Path: ".to_string());

            let mut buf = self.opts.root.to_string_lossy();
            let output = egui::TextEdit::singleline(&mut buf).code_editor().show(ui);
            if output.response.changed() {
                self.opts.root = buf.to_string().into();
            }

            if ui.button("Choose...").clicked() {
                if let Some(path) = rfd::FileDialog::new()
                    .set_directory(&self.opts.root)
                    .pick_folder() {
                    self.opts.root = path;
                }
            }
        });

        ui.separator();

        ui.collapsing("Advanced", |ui| {
            egui::Grid::new(0).num_columns(2).show(ui, |ui| {
                ui.label("Follow Symlinks:");
                ui.checkbox(&mut self.opts.follow_sym, "");
                ui.end_row();

                ui.label("Num Worker Threads:");
                let textedit = TextEdit::singleline(&mut self.opts.num_worker_threads)
                    .desired_width(Self::NUM_WORKER_THREADS_WIDTH);
                ui.add(textedit);
                ui.end_row();

                ui.label("Extensions:");
                let textedit = TextEdit::singleline(&mut self.opts.exts)
                    .desired_width(f32::INFINITY);
                ui.add(textedit);
                ui.end_row();

                ui.label("Supported Extensions:");
                ui.label(SUPPORTED_EXTS.iter().join(","));
                ui.end_row();

            });
        });

        ui.separator();

        if ui.button("Search").clicked() 
            || ui.input(|i| i.key_pressed(egui::Key::Enter)) {
            return self.make_searching_phase().map(Some);
        }

        Ok(None)
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        storage.set_string(Self::ROOT_KEY, self.opts.root.to_string_lossy().into());
    }
}

