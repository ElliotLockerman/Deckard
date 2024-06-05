
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // hide console window on Windows in release

mod startup_phase;
mod searching_phase;
mod output_phase;
mod searcher;
mod misc;

use startup_phase::StartupPhase;

use eframe::egui;

const MIN_INNER_SIZE: (f32, f32) = (550.0, 400.0);

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
    fn save(&mut self, _storage: &mut dyn eframe::Storage) {}
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
    fn draw(&self) {
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
            let action = self.phase.render(ctx, ui);
            match action {
                Action::None => (),
                Action::Trans(next) => self.phase = next,
                Action::Modal(modal) => {
                    // Shouldn't be possible if Action::Modal is only returned
                    // respose to a user action (since users can't interact with
                    // a Phase-controlled widget while a modal is shown).
                    modal.draw();
                },
            }
        });
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        self.phase.save(storage);
    }
}

////////////////////////////////////////////////////////////////////////////////

fn main() -> Result<(), eframe::Error> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_min_inner_size(MIN_INNER_SIZE),
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

