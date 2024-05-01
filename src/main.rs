
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // hide console window on Windows in release

use std::path::PathBuf;
use std::thread;
use std::io::Read;

use egui::load::Bytes;

use eframe::egui;

use egui_extras::{TableBuilder, Column};

mod search;


enum Mode {
    Setup,
    Running,
    Output,
}

struct Image {
    path: PathBuf,
    handle: String,
    buffer: Bytes,
}

struct App {
    mode: Mode,
    root: PathBuf,
    thread: Option<std::thread::JoinHandle<Vec<Vec<PathBuf>>>>,
    images: Option<Vec<Vec<Image>>>,
}

fn starting_root() -> PathBuf {
    match homedir::get_my_home() {
        Ok(path_opt) => path_opt.unwrap_or(PathBuf::from("/")),
        Err(_) => PathBuf::from("/"),
    }
}

impl App {
    fn new() -> App {
        App {
            mode: Mode::Setup,
            root: starting_root(),
            thread: None,
            images: None,
        }
    }

    fn startup(&mut self, _ctx: &egui::Context, ui: &mut egui::Ui) {
        assert!(self.thread.is_none());
        assert!(self.images.is_none());
        ui.heading("DupFind");

        ui.separator();

        ui.label(format!("Root: {}", self.root.display()));

        if ui.button("Choose root...").clicked() {
            if let Some(path) = rfd::FileDialog::new().pick_folder() {
                self.root = path;
            }
        }

        ui.separator();

        if ui.button("Search").clicked() {
            self.mode = Mode::Running;
            let root = self.root.clone();
            self.thread = Some(thread::spawn(move ||
                search::search(root, false, None, None)
            ));
        }
    }

    fn running(&mut self, _ctx: &egui::Context, ui: &mut egui::Ui) {
        if self.thread.is_some() {
            let done = self.thread.as_ref().unwrap().is_finished();
            if done {
                let thread = self.thread.take().unwrap();
                let paths = thread.join().unwrap();
                let mut images = vec![];
                // TODO: do this in a thread?
                for dups in &paths {
                    let mut vec = vec![];
                    for path in dups {
                        let mut buffer = vec![];
                        // Manually loading the image and passing it as bytes is the only way I could get it to handle URIs with spaces
                        std::fs::File::open(path.clone()).unwrap().read_to_end(&mut buffer).unwrap();
                        vec.push(Image{
                            path: path.clone(),
                            handle: format!("{}", path.display()),
                            buffer: egui::load::Bytes::from(buffer),
                        });
                    }
                    images.push(vec);
                }
                self.images = Some(images);
                self.mode = Mode::Output;
                return;
            }
        } else {
            panic!("Where is my thread?");
        }

        ui.heading("DupFind");

        ui.separator();

        ui.label(format!("Running on {}...", self.root.display()));

    }

    fn output(&mut self, _ctx: &egui::Context, ui: &mut egui::Ui) {
        ui.heading("DupFind");

        ui.separator();

        if ui.button("<- New Search").clicked() {
            self.images = None;
            self.mode = Mode::Setup;
            return;
        }

        if let Some(images) = self.images.as_ref() {
            if images.len() > 0 {
                let clicked = App::draw_results_table(ui, images);
                if let Some((set, idx)) = clicked {
                    println!("Clicked {}", images[set][idx].path.display());
                }
            }
        } else {
            ui.label(format!("Done on {}, found no dups", self.root.display()));
        }

        ui.separator();
    }

    // Returns clicked (dup_idx, row)
    fn draw_results_table(ui: &mut egui::Ui, images: &Vec<Vec<Image>>) -> Option<(usize, usize)> {
        let mut clicked = None;
        egui::ScrollArea::both().show(ui, |ui| {
            for (dup_idx, dups) in images.iter().enumerate() {
                ui.separator();
                ui.push_id(dup_idx, |ui| {
                    TableBuilder::new(ui)
                        .column(Column::remainder().resizable(true))
                        .column(Column::auto().resizable(true))
                        .sense(egui::Sense::click())
                        .vscroll(false)
                        .body(|body| {
                            body.rows(100.0, dups.len(), |mut row| {
                                let idx = row.index(); 
                                let image = &dups[idx];
                                row.col(|ui| {
                                    if ui.label(format!("{}", image.path.display())).clicked() {
                                        clicked = Some((dup_idx, idx));
                                    }
                                });
                                row.col(|ui| {
                                    ui.add(egui::Image::from_bytes(image.handle.clone(), image.buffer.clone()));
                                });
                                if row.response().clicked() {
                                    clicked = Some((dup_idx, idx));
                                }
                            });
                        });
                });
            }
        });
        clicked
    }

}


impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui_extras::install_image_loaders(ctx);
        egui::CentralPanel::default().show(ctx, |ui| {
            match self.mode {
                Mode::Setup => self.startup(ctx, ui),
                Mode::Running => self.running(ctx, ui),
                Mode::Output => self.output(ctx, ui),
            }
        });
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
