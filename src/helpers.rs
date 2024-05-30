
use std::path::Path;
use std::process::Command;

use eframe::egui;

use egui_modal::Modal;

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


pub struct ModalContents {
    title: String,
    body: String,
}

impl ModalContents {
    pub fn new(title: String, body: String) -> ModalContents {
        ModalContents{title, body}
    }
}

// Returns true if closed
pub fn draw_error_modal(ctx: &egui::Context, contents: &ModalContents) -> bool {
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


