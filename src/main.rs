
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // hide console window on Windows in release

mod startup_phase;
mod searching_phase;
mod output_phase;
mod searcher;
mod misc;

use std::sync::Arc;

use startup_phase::StartupPhase;

use eframe::egui;
use egui::viewport::IconData;

const MIN_INNER_SIZE: (f32, f32) = (550.0, 400.0);
const ROOT_KEY: &str = "STARTUPPHASE_ROOT";

type DynPhase = Box<dyn Phase>;

trait Phase {
    // Returns Ok(Some(next_phase)) for next phase to transition to, Ok(None)
    // for a successful render with no phase transition, or an error to be
    // displayed in a modal dialog. The Phase must be in valid state when returning
    // an error, as render() will be called once the modal is dismissed.
    fn render(&mut self, ctx: &egui::Context, ui: &mut egui::Ui) -> Result<Option<DynPhase>>;
    fn save(&mut self, _storage: &mut dyn eframe::Storage) {}
}

////////////////////////////////////////////////////////////////////////////////

struct Error {
    err: String,
    detail: String,
}

impl Error {
    fn new(err: String, detail: String) -> Error {
        Error{err, detail}
    }

    // Blocks until "Ok" is clicked.
    fn show_modal(&self) {
        rfd::MessageDialog::new()
            .set_level(rfd::MessageLevel::Error)
            .set_title(self.err.clone())
            .set_description(self.detail.clone())
            .set_buttons(rfd::MessageButtons::Ok)
            .show();
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}: {}", self.err, self.detail)
    }
}

type Result<T, E = Error> = std::result::Result<T, E>;

////////////////////////////////////////////////////////////////////////////////

struct App {
    phase: DynPhase,
}

impl App {

    fn new(cc: &eframe::CreationContext) -> App {
        App {
            phase: Box::new(StartupPhase::new_with_cc(cc)),
        }
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            let ret = self.phase.render(ctx, ui);
            match ret {
                Ok(Some(next_phase)) => self.phase = next_phase,
                Ok(None) => (),
                Err(err) => err.show_modal(),
            }
        });
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        self.phase.save(storage);
    }
}

////////////////////////////////////////////////////////////////////////////////

fn load_icon() -> Arc<IconData> {
    let image_ret = image::load_from_memory(include_bytes!("../app_files/icon.png"))
        .map(|x| x.into_rgb8());

    let image = match image_ret {
        Ok(x) => x,
        Err(_) => {
            // TODO: logging
            return std::sync::Arc::new(egui::viewport::IconData::default());
        },
    };

    let (width, height) = image.dimensions();
    let data = IconData {
        rgba: image.into_raw(),
        width,
        height,
    };

    Arc::new(data)
}

fn main() -> Result<(), eframe::Error> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_min_inner_size(MIN_INNER_SIZE)
            .with_icon(load_icon()),
        ..Default::default()
    };
    eframe::run_native(
        "Deckard",
        options,
        Box::new(|cc| {
            egui_extras::install_image_loaders(&cc.egui_ctx);
            Box::new(App::new(cc))
        }),
    )
}

