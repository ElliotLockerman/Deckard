
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // hide console window on Windows in release

mod startup_phase;
mod searching_phase;
mod output_phase;
mod search;

use startup_phase::StartupPhase;

use std::path::PathBuf;
use std::io::Read;

use egui::load::Bytes;
use eframe::egui;
use egui_modal;

enum Action {
    None,
    Trans(Box<dyn Phase>),
    Modal(Modal),
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
        // Manually loading the image and passing it as bytes is the only way I could get it to handle URIs with spaces
        let mut buffer = vec![];
        let mut file = match std::fs::File::open(path.clone()) {
            Ok(x) => x,
            Err(e) => {
                return Err(format!("Error opening {}: {}", path.display(), e.to_string()));

            }
        };
        if let Err(e) = file.read_to_end(&mut buffer) {
            return Err(format!("Error reading {}: {}", path.display(), e.to_string()));

        }
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

    // Returns true if closed
    fn draw(&self, ctx: &egui::Context) -> bool {
        let modal = egui_modal::Modal::new(ctx, "error_modal");
        let mut close_clicked = false;
        modal.show(|ui| {
            modal.title(ui, &self.title);
            modal.frame(ui, |ui| {
                modal.body(ui, &self.body);
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
}

////////////////////////////////////////////////////////////////////////////////

struct App {
    phase: Box<dyn Phase>,
    modal: Option<Modal>,
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


        if let Some(modal) = &self.modal {
            if modal.draw(ctx) {
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

