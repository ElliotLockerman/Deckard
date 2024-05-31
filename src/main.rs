
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
    thread: Option<std::thread::JoinHandle<search::SearchResults>>,
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
        assert!(self.thread.is_none());

        ui.label(format!("Root: {}", self.root.display()));

        if ui.button("Choose root...").clicked() {
            if let Some(path) = rfd::FileDialog::new().pick_folder() {
                self.root = path;
            }
        }

        if ui.button("Search").clicked() {
            self.start_running();
        }
    }

    fn phase_running(&mut self, _ctx: &egui::Context, ui: &mut egui::Ui) {
        if self.thread.is_some() {
            let done = self.thread.as_ref().unwrap().is_finished();
            if done {
                let images = self.load_images();
                self.start_output(images);
                return;
            }
        } else {
            panic!("Where is my thread?");
        }

        ui.label(format!("Running on {}...", self.root.display()));
        ui.spinner();
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
        assert!(self.thread.is_none());
        self.errors.clear();
        self.images = None;
        self.phase = Phase::Startup;
    }

    fn start_running(&mut self) {
        assert!(self.phase == Phase::Startup);
        self.phase = Phase::Running;
        let root = self.root.clone();
        self.thread = Some(thread::spawn(move ||
            search::search(root, false, None, None)
        ));
    }

    fn start_output(&mut self, images: Vec<Vec<Image>>) {
        assert!(self.phase == Phase::Running);
        assert!(self.thread.is_none());
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
                                    ui.heading(format!("{}", image.path.display()));
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
        let thread = self.thread.take().unwrap();
        assert!(thread.is_finished());
        let results = thread.join().unwrap();

        self.errors.extend(results.errors);
        let paths = results.duplicates;

        let mut images = vec![];
        // TODO: do this in a thread?
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

                let dimm = if let Ok(img) = image::load_from_memory(&buffer) {
                    Some((img.width(), img.height()))
                } else {
                    None
                };

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
