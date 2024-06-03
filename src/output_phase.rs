
use crate::{Phase, Action, Image, Modal};
use crate::startup_phase::{StartupPhase, UserOpts};

use std::path::Path;
use std::process::Command;

use eframe::egui;
use egui_extras::{TableBuilder, Column};

use humansize::{format_size, DECIMAL};

// For sizes much smaller than 100, the second column (of images) don't end up
// aligned across tables.
const ROW_HEIGHT: f32 = 100.0;

// Eyeballed
const PRE_HEADER_SPACE: f32 = 5.0;

// Eyeballed
const HEADER_SIZE: f32 = 13.0;

pub struct OutputPhase {
    opts: UserOpts,
    images: Vec<Vec<Image>>, // [set of duplicates][duplicate in set]
    errors: Vec<String>,
}

impl OutputPhase {
    pub fn new(opts: UserOpts, images: Vec<Vec<Image>>, errors: Vec<String>) -> OutputPhase {
        OutputPhase {
            opts,
            images,
            errors,
        }
    }

    fn draw_output_row(
        mut row: egui_extras::TableRow,
        dups: &[Image]
        ) -> Option<Modal> {

        let mut modal = None;
        let idx = row.index(); 
        let image = &dups[idx];
        row.col(|ui| {

            ui.add_space(PRE_HEADER_SPACE);
            ui.label(
                egui::RichText::new(image.path.display().to_string())
                .monospace()
                .size(HEADER_SIZE)
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
                    // It shouldn't be (reasonably) possible to clobber one
                    // Some modal with another; see comment in draw_output_table().
                    modal = Some(Modal::new(
                            "Error showing file".to_string(),
                            msg,
                    ));
                }
            });
        });
        row.col(|ui| {
            ui.add(egui::Image::from_bytes(
                    image.path.display().to_string(),
                    image.buffer.clone()
            ));
        });

        modal
    }

    // Actually draws multiple tables, one per set of duplicates, but it looks
    // like one big table with multiple sections. Also draws all errors reported
    // by Searcher.
    fn draw_output_table(&mut self,  ui: &mut egui::Ui) -> Option<Modal> {
        let mut modal = None;
        egui::ScrollArea::both().show(ui, |ui| {
            for (dup_idx, dups) in self.images.iter().enumerate() {
                ui.push_id(dup_idx, |ui| {
                    TableBuilder::new(ui)
                        .column(Column::remainder().resizable(true))
                        .column(Column::auto().resizable(true))
                        .vscroll(false)
                        .striped(true)
                        .body(|body| {
                            body.rows(ROW_HEIGHT, dups.len(), |row| {
                                // It shouldn't be (reasonably) possible to overwite
                                // one Some modal with another, since a Some is
                                // only returned in response to a click, and it
                                // would be nigh-impossible to click twice in a
                                // single frame.
                                if let Some(m) = Self::draw_output_row(row, dups) {
                                    modal = Some(m);
                                }
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
        modal
    }
}

impl Phase for OutputPhase {
    fn render(&mut self, _ctx: &egui::Context, ui: &mut egui::Ui) -> Action {
        if ui.button("<- New Search").clicked() {
            let opts = std::mem::take(&mut self.opts);
            return Action::Trans(Box::new(StartupPhase::with_opts(opts)));
        }

        ui.separator();

        let mut modal = None;
        if !self.images.is_empty() {
            modal = self.draw_output_table(ui);
        } else {
            ui.label(format!("Done on {}, found no duplicates", self.opts.root.display()));
        }
        
        match modal {
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
        OpenKind::Reveal => { command.arg("-R"); },
        OpenKind::Open => (),
    };

    let output = command.output().map_err(|e| e.to_string())?;
    if output.status.success() {
        Ok(())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).to_string())
    }
}
