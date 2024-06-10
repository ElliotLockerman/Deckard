use std::path::PathBuf;
use std::io::Read;

use crate::egui::load::Bytes;

#[derive(Clone)]
pub struct Image {
    pub path: PathBuf,
    pub buffer: Bytes,
    pub file_size: usize, // In bytes
    pub dimm: Option<(u32, u32)>, // Width x height
}

impl Image {
    pub fn new(path: PathBuf, buffer: Vec<u8>, dimm: Option<(u32, u32)>) -> Image {
        let file_size = buffer.len();
        Image{
            path,
            buffer: Bytes::from(buffer),
            file_size,
            dimm,
        }
    }

    pub fn load(path: PathBuf) -> Result<Image, String> {
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
