
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // hide console window on Windows in release

use std::path::PathBuf;
use std::io::Read;

use egui::load::Bytes;

use eframe::egui;

use egui_extras::{TableBuilder, Column};

use humansize::{format_size, DECIMAL};

mod search;
mod helpers;

use helpers::*;
use search::Searcher;

struct Image {
    path: PathBuf,
    handle: String,
    buffer: Bytes,
    file_size: usize, // In bytes
    dimm: Option<(u32, u32)>, // Width x height
}

type DynPhase = Box<dyn Phase>;

enum Action {
    None,
    Trans(DynPhase),
    Modal(ModalContents),
}

trait Phase {
    fn render(&mut self, ctx: &egui::Context, ui: &mut egui::Ui) -> Action;
}

////////////////////////////////////////////////////////////////////////////////

struct StartupPhase {
    root: PathBuf,
    follow_sym: bool,
    limit_depth: bool,
    max_depth: String,
    num_worker_threads: String,
}

impl StartupPhase {
    fn new(root: PathBuf) -> StartupPhase {
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
                    return Action::Modal(ModalContents::new(
                        "Error parsing depth limit".to_string(),
                        e.to_string(),
                    ))
                },
            }
            if max_depth == Some(0usize) {
                return Action::Modal(ModalContents::new(
                    "Invalid depth limit".to_string(),
                    "A depth limit of 0 doesn't search at all".to_string(),
                ));
            }
        }

        let num_worker_threads = match self.num_worker_threads.parse::<usize>() {
            Ok(x) => x,
            Err(e) => {
                return Action::Modal(ModalContents::new(
                    "Error parsing num worker threads".to_string(),
                    e.to_string(),
                ));
            },
        };
        if num_worker_threads == 0 {
            return Action::Modal(ModalContents::new(
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

////////////////////////////////////////////////////////////////////////////////

struct SearchingPhase {
    root: PathBuf,
    searcher: Searcher,
}

impl SearchingPhase {
    fn new(root: PathBuf, searcher: Searcher) -> SearchingPhase {
        SearchingPhase {
            root,
            searcher,
        }
    }

    fn make_output_phase(&mut self) -> Box<dyn Phase> {
        assert!(self.searcher.is_finished());
        let results = self.searcher.join();

        let mut errors = results.errors;
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
                        errors.push(format!("Error opening {}: {}", path.display(), e.to_string()));
                        continue;
                    }
                };
                if let Err(e) = file.read_to_end(&mut buffer) {
                    errors.push(format!("Error reading {}: {}", path.display(), e.to_string()));
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

        let root = std::mem::replace(&mut self.root, PathBuf::new());
        Box::new(OutputPhase::new(root, images, errors))
    }
}

impl Phase for SearchingPhase {

    fn render(&mut self, _ctx: &egui::Context, ui: &mut egui::Ui) -> Action {
        if self.searcher.is_finished() {
            if self.searcher.was_canceled() {
                self.searcher.join();
                let root = std::mem::replace(&mut self.root, PathBuf::new());
                return Action::Trans(Box::new(StartupPhase::new(root)));
            } else {
                return Action::Trans(self.make_output_phase());
            }
        }

        if ui.button("<- New Search").clicked() {
            self.searcher.cancel();
        }

        ui.separator();


        ui.horizontal(|ui| {
            ui.strong(format!("Searching"));
            ui.monospace(format!("{}", self.root.display()));
        });

        ui.centered_and_justified(|ui| {
            let spinner = egui::widgets::Spinner::new().size(256.0);
            ui.add(spinner);
        });

        Action::None
    }
}

////////////////////////////////////////////////////////////////////////////////

struct OutputPhase {
    root: PathBuf, // Just so we can go back to startup and keep the entered root
    images: Vec<Vec<Image>>, // [set of duplicates][duplicate in set]
    errors: Vec<String>,
}

impl OutputPhase {
    fn new(root: PathBuf, images: Vec<Vec<Image>>, errors: Vec<String>) -> OutputPhase {
        OutputPhase {
            root,
            images,
            errors,
        }
    }

    fn draw_output_table(&mut self,  ui: &mut egui::Ui) -> Option<ModalContents> {
        let mut modal_contents = None;
        egui::ScrollArea::both().show(ui, |ui| {
            for (dup_idx, dups) in self.images.iter().enumerate() {
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
                                            modal_contents = Some(ModalContents::new(
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
        modal_contents
    }
}

impl Phase for OutputPhase {
    fn render(&mut self, _ctx: &egui::Context, ui: &mut egui::Ui) -> Action {
        if ui.button("<- New Search").clicked() {
            let root = std::mem::replace(&mut self.root, PathBuf::new());
            return Action::Trans(Box::new(StartupPhase::new(root)));
        }

        ui.separator();

        let mut modal = None;
        if !self.images.is_empty() {
            modal = self.draw_output_table(ui);
        } else {
            ui.label(format!("Done on {}, found no duplicates", self.root.display()));
        }
        
        return match modal {
            Some(x) => Action::Modal(x),
            None => Action::None,
        }
    }
}

////////////////////////////////////////////////////////////////////////////////

struct App {
    phase: Box<dyn Phase>,
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
            phase: Box::new(StartupPhase::new(default_root())),
            modal: None,
        }
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            let action = self.phase.render(ctx, ui);

            match action {
                Action::None => return,
                Action::Trans(next) => self.phase = next,
                Action::Modal(modal) => {
                    // TODO: what to do about this constraint?
                    // Hopefully a rule like "only set modal in response to a 
                    // click" will do the trick, seeing as while you're in a 
                    // modal you can't click, but it would be better if it was 
                    // a constraint that can be more gently enforced.
                    assert!(!self.modal.is_some());
                    self.modal = Some(modal);
                },
            }
        });


        if let Some(contents) = &self.modal {
            if draw_error_modal(ctx, &contents) {
                self.modal = None;
            }
        }
    }
}

////////////////////////////////////////////////////////////////////////////////

fn main() -> Result<(), eframe::Error> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([800.00, 600.0]),
        ..Default::default()
    };
    eframe::run_native(
        "DupFind",
        options,
        Box::new(|cc| {
            egui_extras::install_image_loaders(&cc.egui_ctx);
            Box::new(App::new())
        }),
    )
}
