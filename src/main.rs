
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // hide console window on Windows in release

use std::path::PathBuf;
use std::thread;
use std::io::Read;

use egui::load::Bytes;

use eframe::egui;

use egui_extras::{TableBuilder, Column};

use humansize::{format_size, DECIMAL};

mod search;
mod helpers;

use helpers::*;
use search::Searcher;


#[derive(PartialEq, Eq)]
enum Phase {
    Startup,
    Running,
    Output,
}

struct Image {
    path: PathBuf,
    handle: String,
    buffer: Bytes,
    file_size: usize, // In bytes
    dimm: Option<(u32, u32)>, // Width x height
}

struct App {
    phase: Phase,
    root: PathBuf,
    follow_sym: bool,
    limit_depth: bool,
    max_depth: String,
    num_worker_threads: String,
    thread: Option<std::thread::JoinHandle<search::SearchResults>>,
    searcher: Option<Searcher>,
    images: Option<Vec<Vec<Image>>>,
    errors: Vec<String>,
    modal: Option<ModalContents>,
}

fn default_root() -> PathBuf {
    match homedir::get_my_home() {
        Ok(path_opt) => path_opt.unwrap_or(PathBuf::from("/")),
        Err(_) => PathBuf::from("/"),
    }
}

impl App {
    fn new() -> App {
        App {
            phase: Phase::Startup,
            root: default_root(),
            follow_sym: true,
            limit_depth: false,
            max_depth: String::new(),
            num_worker_threads: num_cpus::get().to_string(),
            searcher: None,
            thread: None,
            images: None,
            errors: vec![],
            modal: None,
        }
    }

////////////////////////////////////////////////////////////////////////////////
// Phase Core Functions
////////////////////////////////////////////////////////////////////////////////

    fn phase_startup(&mut self, _ctx: &egui::Context, ui: &mut egui::Ui) {
        assert!(self.searcher.is_none());

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
            self.start_running();
        }
    }

    fn phase_running(&mut self, _ctx: &egui::Context, ui: &mut egui::Ui) {
        let searcher = self.searcher.as_ref().expect("searcher missing");
        if searcher.is_finished() {
            if self.searcher.as_ref().expect("missing searcher").was_canceled() {
                self.searcher.take().unwrap().join();
                self.start_startup();
            } else {
                let images = self.load_images();
                self.start_output(images);
            }
            return;
        }

        if ui.button("<- New Search").clicked() {
            self.searcher.as_ref().expect("missing searcher").cancel();
        }

        ui.separator();


        ui.horizontal(|ui| {
            ui.strong(format!("Running on"));
            ui.monospace(format!("{}", self.root.display()));
        });

        ui.centered_and_justified(|ui| {
            let spinner = egui::widgets::Spinner::new().size(256.0);
            ui.add(spinner);
        });
    }

    fn phase_output(&mut self, _ctx: &egui::Context, ui: &mut egui::Ui) {
        if ui.button("<- New Search").clicked() {
            self.start_startup();
            return;
        }

        ui.separator();

        if let Some(images) = self.images.as_ref() {
            if !images.is_empty() {
                self.draw_output_table(ui);
            } else {
                ui.label(format!("Done on {}, found no duplicates", self.root.display()));
            }
        } else {
            ui.label(format!("Error, no results found"));
        }
    }

////////////////////////////////////////////////////////////////////////////////
// Phase Transitions
////////////////////////////////////////////////////////////////////////////////

    fn start_startup(&mut self) {
        assert!(self.searcher.is_none());
        assert!(self.modal.is_none());
        self.errors.clear();
        self.images = None;
        self.phase = Phase::Startup;
    }

    fn start_running(&mut self) {
        assert!(self.phase == Phase::Startup);
        assert!(self.searcher.is_none());
        assert!(self.modal.is_none());

        let mut max_depth = None;
        if self.limit_depth {
            match self.max_depth.parse::<usize>() {
                Ok(x) => max_depth = Some(x),
                Err(e) => {
                    self.modal = Some(ModalContents::new(
                        "Error parsing depth limit".to_string(),
                        e.to_string(),
                    ));
                    return;
                },
            }
            if max_depth == Some(0usize) {
                self.modal = Some(ModalContents::new(
                    "Invalid depth limit".to_string(),
                    "A depth limit of 0 doesn't search at all".to_string(),
                ));
                return;
            }
        }

        let num_worker_threads = match self.num_worker_threads.parse::<usize>() {
            Ok(x) => x,
            Err(e) => {
                self.modal = Some(ModalContents::new(
                    "Error parsing num worker threads".to_string(),
                    e.to_string(),
                ));
                return;
            },
        };
        if num_worker_threads == 0 {
            self.modal = Some(ModalContents::new(
                "Invalid num worker threads".to_string(),
                "At least 1 worker thread is required".to_string(),
            ));
            return;
        }

        let mut searcher = Searcher::new(
            self.root.clone(),
            self.follow_sym,
            num_worker_threads,
            max_depth,
        );
        searcher.launch_search();
        self.searcher = Some(searcher);
        self.phase = Phase::Running;
    }

    fn start_output(&mut self, images: Vec<Vec<Image>>) {
        assert!(self.phase == Phase::Running);
        assert!(self.searcher.is_none());
        assert!(self.modal.is_none());
        self.images = Some(images);
        self.phase = Phase::Output;
    }

////////////////////////////////////////////////////////////////////////////////
// Drawing Helpers
////////////////////////////////////////////////////////////////////////////////

    fn draw_output_table(&mut self,  ui: &mut egui::Ui) {
        assert!(self.images.is_some());
        egui::ScrollArea::both().show(ui, |ui| {
            for (dup_idx, dups) in self.images.as_ref().unwrap().iter().enumerate() {
                ui.push_id(dup_idx, |ui| {
                    TableBuilder::new(ui)
                        .column(Column::remainder().resizable(true))
                        .column(Column::auto().resizable(true))
                        .vscroll(false)
                        .striped(true)
                        .body(|body| {
                            body.rows(100.0, dups.len(), |mut row| {
                                let idx = row.index(); 
                                let image = &dups[idx];
                                row.col(|ui| {
                                    ui.label(
                                        egui::RichText::new(format!("{}", image.path.display()))
                                            .monospace()
                                            .size(13.0)
                                    );
                                    if let Some((width, height)) = image.dimm {
                                        ui.label(format!("{width}×{height}"));
                                    }
                                    ui.label(format_size(image.file_size, DECIMAL));
                                    ui.horizontal(|ui| {
                                        let err = if ui.button("Open").clicked() {
                                            open_file(image.path.as_path(), OpenKind::Open)
                                        } else if ui.button("Show").clicked() {
                                            open_file(image.path.as_path(), OpenKind::Reveal)
                                        } else {
                                            Ok(())
                                        };

                                        if let Err(msg) = err {
                                            self.modal = Some(ModalContents::new(
                                                "Error showing file".to_string(),
                                                msg,
                                            ));
                                        }
                                    });
                                });
                                row.col(|ui| {
                                    ui.add(egui::Image::from_bytes(
                                            image.handle.clone(),
                                            image.buffer.clone()
                                        )
                                    );
                                });
                            });
                        });
                });
                ui.separator();
            }

            if !self.errors.is_empty() {
                ui.heading(egui::RichText::new("Errors").color(egui::Color32::RED));
                for err in &self.errors {
                    ui.label(err);
                }
            }
        });
    }

    fn draw_modal(&mut self, ctx: &egui::Context) {
        if let Some(contents) = &self.modal {
            if draw_error_modal(ctx, &contents) {
                self.modal = None;
            }
        }
    }

////////////////////////////////////////////////////////////////////////////////

    fn load_images(&mut self) -> Vec<Vec<Image>> {
        assert!(self.searcher.is_some());
        assert!(self.searcher.as_ref().unwrap().is_finished());
        let results = self.searcher.take().unwrap().join();

        self.errors.extend(results.errors);
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
                        self.errors.push(format!("Error opening {}: {}", path.display(), e.to_string()));
                        continue;
                    }
                };
                if let Err(e) = file.read_to_end(&mut buffer) {
                    self.errors.push(format!("Error reading {}: {}", path.display(), e.to_string()));
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
        images
    }
}


impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui_extras::install_image_loaders(ctx);
        egui::CentralPanel::default().show(ctx, |ui| {
            match self.phase {
                Phase::Startup => self.phase_startup(ctx, ui),
                Phase::Running => self.phase_running(ctx, ui),
                Phase::Output => self.phase_output(ctx, ui),
            }
        });
        self.draw_modal(ctx);
    }
}

fn main() -> Result<(), eframe::Error> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([800.00, 600.0]),
        ..Default::default()
    };
    eframe::run_native(
        "DupFind",
        options,
        Box::new(|_cc| Box::new(App::new())),
    )
}
