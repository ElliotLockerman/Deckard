
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // hide console window on Windows in release

use std::path::PathBuf;
use std::thread;

use eframe::egui;

use egui_extras::{TableBuilder, Column};

mod search;


enum Mode {
    Setup,
    Running,
    Output,
}

struct App {
    mode: Mode,
    root: PathBuf,
    thread: Option<std::thread::JoinHandle<Vec<Vec<PathBuf>>>>,
    results: Option<Vec<Vec<PathBuf>>>,
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
            results: None,
        }
    }

    fn startup(&mut self, _ctx: &egui::Context, ui: &mut egui::Ui) {
        assert!(self.thread.is_none());
        assert!(self.results.is_none());
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
                self.results = Some(thread.join().unwrap());
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
            self.results = None;
            self.mode = Mode::Setup;
            return;
        }

        if let Some(results) = self.results.as_ref() {
            if results.len() > 0 {
                let clicked = App::draw_results_table(ui, results);
                if let Some((dup_idx, row)) = clicked {
                    eprintln!("Clicked {dup_idx}:{row}");
                }
            }
        } else {
            ui.label(format!("Done on {}, found no dups", self.root.display()));
        }
    }

    // Returns clicked (dup_idx, row)
    fn draw_results_table(ui: &mut egui::Ui, results: &Vec<Vec<PathBuf>>) -> Option<(usize, usize)> {
        let mut clicked = None;
        egui::ScrollArea::vertical().show(ui, |ui| {
            for (dup_idx, dups) in results.iter().enumerate() {
                ui.separator();
                ui.push_id(dup_idx, |ui| {
                    TableBuilder::new(ui)
                        .column(Column::remainder().resizable(true))
                        .sense(egui::Sense::click())
                        .body(|body| {
                            body.rows(20.0, dups.len(), |mut row| {
                                let idx = row.index(); 
                                row.col(|ui| {
                                    if ui.label(format!("{}", dups[idx].display())).clicked() {
                                        clicked = Some((dup_idx, idx));
                                    }
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
            // .with_drag_and_drop(true),
        ..Default::default()
    };
    eframe::run_native(
        "DupFind",
        options,
        Box::new(|_cc| Box::new(App::new())),
    )
}
