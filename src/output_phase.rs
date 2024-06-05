
use crate::{Phase, Action, Modal};
use crate::startup_phase::{StartupPhase, UserOpts};
use crate::misc::Image;

use eframe::egui;

use humansize::{format_size, DECIMAL};


pub struct OutputPhase {
    opts: UserOpts,
    update_count: std::num::Saturating<usize>,
    images: Vec<Vec<Image>>, // [set of duplicates][duplicate in set]
    errors: Vec<String>,
}

impl OutputPhase {

    // Eyeballed
    const HEADER_SIZE: f32 = 13.0;
    const MIN_CELL_SIZE: f32 = 150.0;
    const H_SPACING: f32 = 20.0;
    const CELL_2_TOP_SPACING: f32 = 10.0;

    pub fn new(opts: UserOpts, images: Vec<Vec<Image>>, errors: Vec<String>) -> OutputPhase {
        OutputPhase {
            opts,
            update_count: std::num::Saturating(0usize),
            images,
            errors,
        }
    }

    fn draw_output_row(
        &self,
        ui: &mut egui::Ui,
        row: usize,
        image: &Image
    ) -> Result<(), Modal> {

        let mut modal = Ok(());

        // Space out showing the images instead of blocking untill they're all
        // ready (they're slow to show for the first time).
        if self.update_count.0 >= row {
            let resp = ui.add(egui::widgets::ImageButton::new(egui::Image::from_bytes(
                    image.path.display().to_string(),
                    image.buffer.clone()
            )));

            if resp.clicked() {
                if let Err(e) = opener::open(&image.path) {
                    modal = Err(Modal::new(
                            "Error showing file".to_string(),
                            e.to_string(),
                    ));
                }
            }
        }

        ui.vertical(|ui| {
            let stripped = image.path.strip_prefix(&self.opts.root).unwrap_or(&image.path);

            ui.add_space(Self::CELL_2_TOP_SPACING);

            ui.label(
                egui::RichText::new(stripped.display().to_string())
                .monospace()
                .size(Self::HEADER_SIZE)
            );
            if let Some((width, height)) = image.dimm {
                ui.label(format!("{width}×{height}"));
            }
            ui.label(format_size(image.file_size, DECIMAL));
            ui.horizontal(|ui| {
                let err = if ui.button("Open").clicked() {
                    opener::open(&image.path)
                } else if ui.button("Show").clicked() {
                    opener::reveal(&image.path)
                } else {
                    Ok(())
                };

                if let Err(e) = err {
                    // It shouldn't be (reasonably) possible to clobber one
                    // Some modal with another; see comment in draw_output_table().
                    modal = Err(Modal::new(
                            "Error showing file".to_string(),
                            e.to_string(),
                    ));
                }

                if ui.button("Copy path").clicked() {
                    ui.output_mut(|out| 
                        out.copied_text = image.path.as_os_str().to_string_lossy().to_string()
                    );
                }
            });
        });

        modal
    }

    // Actually draws multiple tables, one per set of duplicates, but it looks
    // like one big table with multiple sections. Also draws all errors reported
    // by Searcher.
    fn draw_output_table(&mut self,  ui: &mut egui::Ui) -> Result<(), Modal> {
        let mut modal = Ok(());

        let mut scroll = egui::ScrollArea::vertical().drag_to_scroll(false);

        // Scroll offset is persistent, and I can't find a way to opt-out for
        // a single widget. This overrides it manually.
        if self.update_count.0 == 0 {
            scroll = scroll.vertical_scroll_offset(0.0);
        }

        scroll.show(ui, |ui| {
            for (dup_idx, dups) in self.images.iter().enumerate() {
                egui::Grid::new(dup_idx)
                    .striped(true)
                    .min_col_width(Self::MIN_CELL_SIZE)
                    .min_row_height(Self::MIN_CELL_SIZE)
                    .spacing((Self::H_SPACING, ui.spacing().item_spacing.y))
                    .num_columns(2)
                    .show(ui, |ui| {

                    for image in dups {
                        if let Err(m) = self.draw_output_row(ui, dup_idx, image) {
                            modal = Err(m);
                        }
                        ui.end_row();
                    }
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
        let resp = ui.horizontal(|ui| {
            if ui.button("<- New Search").clicked() {
                let opts = std::mem::take(&mut self.opts);
                return Some(Action::Trans(Box::new(StartupPhase::new_with_opts(opts))));
            }

            ui.label("Results for");
            ui.label(
                egui::RichText::new(self.opts.root.display().to_string())
                .monospace()
            );

            None
        });

        if let Some(action) = resp.inner {
            return action;
        }

        ui.separator();

        if self.images.is_empty() {
            ui.label(format!("Done on {}, found no duplicates", self.opts.root.display()));
        }

        let action = match self.draw_output_table(ui) {
            Ok(()) => Action::None,
            Err(modal) => Action::Modal(modal),
        };

        self.update_count += 1;

        action
    }
}

