
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::collections::HashSet;

use crate::ROOT_KEY;
use crate::{Phase, DynPhase, Result, Error};
use crate::startup_phase::{StartupPhase, UserOpts};
use crate::misc::Image;

use eframe::egui;

use humansize::{format_size, DECIMAL};


pub struct OutputPhase {
    opts: UserOpts,
    first_update: bool,
    images: Vec<Vec<Image>>, // [set of duplicates][duplicate in set]
    flattened_images: Vec<Image>,
    last_indices: HashSet<usize>, // Index in flattened_images of last image in hash bucket
    errors: Vec<String>,
    show_errors: Arc<AtomicBool>,
}

impl OutputPhase {

    // Eyeballed
    const HEADER_SIZE: f32 = 13.0;
    const MIN_CELL_SIZE: f32 = 150.0;
    const H_SPACING: f32 = 20.0;
    const CELL_2_TOP_SPACING: f32 = 15.0;
    const CELL_2_BOTTOM_SPACING: f32 = 15.0;
    const CELL_2_DATA_SPACING: f32 = 3.0;

    pub fn new(opts: UserOpts, images: Vec<Vec<Image>>, errors: Vec<String>) -> OutputPhase {
        let last_indices = images.iter()
            .scan(usize::MAX, |total, dups| {*total += dups.len(); Some(*total)})
            .collect();

        OutputPhase {
            opts,
            first_update: true,
            flattened_images: images.iter().flat_map(|x| x.clone()).collect(),
            last_indices,
            images,
            errors,
            show_errors: Arc::new(AtomicBool::new(true)),
        }
    }

    pub fn into_dyn(self) -> DynPhase {
        Box::new(self)
    }

    fn draw_output_row(&self, ui: &mut egui::Ui, image: &Image, last_in_group: bool) -> Result<()> {

        let mut ret = Ok(());

        let resp = ui.centered_and_justified(|ui| {
            let resp = ui.add(egui::widgets::ImageButton::new(egui::Image::from_bytes(
                    image.path.display().to_string(),
                    image.buffer.clone()
            )));
            if last_in_group {
                ui.separator();
            }
            resp
        });

        if resp.inner.clicked() {
            if let Err(e) = opener::open(&image.path) {
                ret = Err(Error::new(
                        "Error showing file".to_string(),
                        e.to_string(),
                ));
            }
        }

        ui.with_layout(egui::Layout::top_down_justified(egui::Align::Min), |ui| {
            let stripped = image.path.strip_prefix(&self.opts.root).unwrap_or(&image.path);

            ui.add_space(Self::CELL_2_TOP_SPACING);

            ui.label(
                egui::RichText::new(stripped.display().to_string())
                .monospace()
                .size(Self::HEADER_SIZE)
            );
            ui.add_space(Self::CELL_2_DATA_SPACING);
            if let Some((width, height)) = image.dimm {
                ui.label(format!("{width}×{height}"));
                ui.add_space(Self::CELL_2_DATA_SPACING);
            }
            ui.label(format_size(image.file_size, DECIMAL));

            ui.with_layout(egui::Layout::bottom_up(egui::Align::Min), |ui| {
                let sep_height = if last_in_group {
                    ui.separator().rect.height().clamp(0.0, Self::CELL_2_BOTTOM_SPACING)
                } else {
                    0.0
                };
                ui.add_space(Self::CELL_2_BOTTOM_SPACING - sep_height);
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
                        ret = Err(Error::new(
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
        });

        ret
    }

    // Actually draws multiple tables, one per set of duplicates, but it looks
    // like one big table with multiple sections. Also draws all errors reported
    // by Searcher.
    fn draw_output_table(&mut self, ui: &mut egui::Ui) -> Result<()> {
        let mut ret = Ok(());

        let mut scroll = egui::ScrollArea::vertical().drag_to_scroll(false);

        // Scroll offset is persistent, and I can't find a way to opt-out for
        // a single widget. This overrides it manually.
        if self.first_update {
            scroll = scroll.vertical_scroll_offset(0.0);
            self.first_update = false;
        }

        let total_rows = self.flattened_images.len();
        scroll.show_rows(ui, Self::MIN_CELL_SIZE, total_rows, |ui, range| {
            egui::Grid::new(0)
                .striped(true)
                .min_col_width(Self::MIN_CELL_SIZE)
                .min_row_height(Self::MIN_CELL_SIZE)
                .spacing((Self::H_SPACING, ui.spacing().item_spacing.y))
                .num_columns(2)
                .show(ui, |ui| {

                for idx in range {
                    let last = self.last_indices.contains(&idx);
                    if let Err(m) = self.draw_output_row(ui, &self.flattened_images[idx], last) {
                        ret = Err(m);
                    }
                    ui.end_row();
                }
            });
        });

        ret
    }

    fn draw_errors(&mut self, ctx: &egui::Context) {
        if self.errors.is_empty() || !self.show_errors.load(Ordering::Relaxed) {
            return;
        }

        let vb = egui::viewport::ViewportBuilder::default().with_title("Errors");
        let vid = egui::viewport::ViewportId::from_hash_of("error window");
        let show_errors = self.show_errors.clone();
        let errors = self.errors.clone();
        ctx.show_viewport_deferred(vid, vb, move |ctx, _| {
            egui::CentralPanel::default().show(ctx, |ui| {
                if ctx.input(|i| i.viewport().close_requested()) {
                    show_errors.store(false, Ordering::Relaxed);
                    return;
                }

                egui::ScrollArea::vertical().drag_to_scroll(false).show(ui, |ui| {
                    ui.heading(egui::RichText::new("Errors").color(egui::Color32::RED));
                    for err in &errors {
                        ui.label(err);
                    }
                });
            });
        });
    }
}

impl Phase for OutputPhase {
    fn render(&mut self, ctx: &egui::Context, ui: &mut egui::Ui) -> Result<Option<DynPhase>> {
        let resp = ui.horizontal(|ui| {
            if ui.button("<- New Search").clicked() {
                return Some(StartupPhase::new_with_opts(self.opts.take()).into_dyn());
            }

            ui.strong("Results for");
            ui.monospace(self.opts.root.display().to_string());

            None
        });
        
        if resp.inner.is_some() {
            return Ok(resp.inner);
        }

        // A button to toggle showing the error window (if there were any errors).
        // I can't figure out where to put the button, and I'm not sure its really
        // necessary, but I'm leaving it here for the future.
        /*
        if !self.errors.is_empty() {
            ui.add_space(4.0);
            let old_show_errors = self.show_errors.load(Ordering::Relaxed);
            let text = if old_show_errors { "Hide Errors" } else { "Show Errors" };
            if ui.button(text).clicked() {
                // TODO: change to fetch_not() once stable.
                self.show_errors.store(!old_show_errors, Ordering::Relaxed);
            }
        }
        */

        ui.separator();

        if self.images.is_empty() {
            ui.label(format!("Done on {}, found no duplicates", self.opts.root.display()));
        }

        self.draw_output_table(ui)?;
        self.draw_errors(ctx);

        Ok(None)
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        storage.set_string(ROOT_KEY, self.opts.root.to_string_lossy().into());
    }
}

