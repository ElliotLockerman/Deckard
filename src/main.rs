
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // hide console window on Windows in release

mod startup_phase;
mod searching_phase;
mod output_phase;
mod searcher;

use startup_phase::StartupPhase;

use std::path::PathBuf;
use std::io::Read;

use egui::load::Bytes;
use eframe::egui;

enum Action {
    None,
    Trans(Box<dyn Phase>),
    Modal(Modal),
}

// Like question mark operator, but takes a Result<T, Modal>, and on error,
// returns an Action::Modal(). For use in functions where the error outcome is 
// an Action::Modal, and the good action is any other Action variant.
#[macro_export]
macro_rules! try_act {
    ($expression:expr) => {
        match $expression {
            Ok(x) => x,
            Err(e) => return Action::Modal(e),
        }
    }
}

trait Phase {
    fn render(&mut self, ctx: &egui::Context, ui: &mut egui::Ui) -> Action;
}


////////////////////////////////////////////////////////////////////////////////

struct Image {
    path: PathBuf,
    buffer: Bytes,
    file_size: usize, // In bytes
    dimm: Option<(u32, u32)>, // Width x height
}

impl Image {
    fn new(path: PathBuf, buffer: Vec<u8>, dimm: Option<(u32, u32)>) -> Image {
        let file_size = buffer.len();
        Image{
            path,
            buffer: egui::load::Bytes::from(buffer),
            file_size,
            dimm,
        }
    }

    fn load(path: PathBuf) -> Result<Image, String> {
        // Manually loading the image and passing it as bytes is the only way I
        // could get it to handle URIs with spaces
        let mut buffer = vec![];
        let mut file = std::fs::File::open(path.clone()).map_err(|e| {
            format!("Error opening {}: {e}", path.display())
        })?;

        file.read_to_end(&mut buffer).map_err(|e| {
            format!("Error reading {}: {e}", path.display())
        })?;

        let dimm = image::load_from_memory(&buffer).ok().map(|img| {
            (img.width(), img.height())
        });
        Ok(Image::new(path.clone(), buffer, dimm))
    }
}


////////////////////////////////////////////////////////////////////////////////

struct Modal {
    title: String,
    body: String,
}

impl Modal {
    fn new(title: String, body: String) -> Modal {
        Modal{title, body}
    }

    // Blocks until "Ok" is clicked.
    fn draw(&self, _ctx: &egui::Context) {
        rfd::MessageDialog::new()
            .set_level(rfd::MessageLevel::Error)
            .set_title(self.title.clone())
            .set_description(self.body.clone())
            .set_buttons(rfd::MessageButtons::Ok)
            .show();
    }
}

////////////////////////////////////////////////////////////////////////////////

struct App {
    phase: Box<dyn Phase>,
    modal: Option<Modal>,
}

fn default_root() -> PathBuf {
    homedir::get_my_home()
        .unwrap_or_else(|_| Some(PathBuf::from("/")))
        .unwrap_or_else(|| PathBuf::from("/"))
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
                Action::None => (),
                Action::Trans(next) => self.phase = next,
                Action::Modal(modal) => {
                    // Shouldn't be possible if Action::Modal is only returned
                    // respose to a user action (since users can't interact with
                    // a Phase-controlled widget while a modal is shown).
                    assert!(self.modal.is_none(), "only one modal allowed at a time");
                    self.modal = Some(modal);
                },
            }
        });


        if let Some(modal) = &self.modal {
            modal.draw(ctx);
            self.modal = None;
        }
    }
}

////////////////////////////////////////////////////////////////////////////////

fn main() -> Result<(), eframe::Error> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default(),
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

