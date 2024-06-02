
use crate::{Phase, Action, Image, Modal};
use crate::startup_phase::StartupPhase;

use std::path::PathBuf;
use std::path::Path;
use std::process::Command;

use eframe::egui;
use egui_extras::{TableBuilder, Column};

use humansize::{format_size, DECIMAL};

pub struct OutputPhase {
    root: PathBuf, // Just so we can go back to startup and keep the entered root
    images: Vec<Vec<Image>>, // [set of duplicates][duplicate in set]
    errors: Vec<String>,
}

impl OutputPhase {
    pub fn new(root: PathBuf, images: Vec<Vec<Image>>, errors: Vec<String>) -> OutputPhase {
        OutputPhase {
            root,
            images,
            errors,
        }
    }

    fn draw_output_table(&mut self,  ui: &mut egui::Ui) -> Option<Modal> {
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
                                            modal_contents = Some(Modal::new(
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

pub enum OpenKind {
    Open,
    Reveal,
}

// TODO: support platforms other than mac
pub fn open_file(path: &Path, open_kind: OpenKind) -> Result<(), String> {
    let mut command = Command::new("open");
    command.arg(path.as_os_str());
    match open_kind {
        OpenKind::Reveal => { command.arg(std::ffi::OsStr::new("-R")); },
        OpenKind::Open => (),
    };
    match command.output() {
        Ok(output) => {
            if output.status.success() {
                Ok(())
            } else {
                Err(String::from_utf8_lossy(&output.stderr).to_string())
            }
        },
        Err(e) => {
            Err(e.to_string())
        }
    }
}
