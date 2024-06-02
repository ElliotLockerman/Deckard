
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // hide console window on Windows in release

mod startup_phase;
mod searching_phase;
mod output_phase;
mod search;

use startup_phase::StartupPhase;

use std::path::PathBuf;

use egui::load::Bytes;
use eframe::egui;
use egui_modal::Modal;


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

struct ModalContents {
    title: String,
    body: String,
}

impl ModalContents {
    fn new(title: String, body: String) -> ModalContents {
        ModalContents{title, body}
    }
}

// Returns true if closed
fn draw_error_modal(ctx: &egui::Context, contents: &ModalContents) -> bool {
    let modal = Modal::new(ctx, "error_modal");
    let mut close_clicked = false;
    modal.show(|ui| {
        modal.title(ui, &contents.title);
        modal.frame(ui, |ui| {
            modal.body(ui, &contents.body);
        });
        modal.buttons(ui, |ui| {
            if modal.button(ui, "Close").clicked() {
                close_clicked = true;
            }
        });
    });
    modal.open();

    close_clicked
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
