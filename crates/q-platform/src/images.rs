//! Image storage and image-clipboard support.
//!
//! Dormant in v1 (text-first per q#11). Unlocked by the image-support plan.

#![allow(dead_code)]

use std::borrow::Cow;
use std::fs;
use std::path::{Path, PathBuf};

use thiserror::Error;

use crate::paths::images_dir;

#[derive(Debug, Error)]
pub enum ImageError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("decode error: {0}")]
    Decode(String),
    #[error("clipboard error: {0}")]
    Clipboard(String),
}

/// Store raw image bytes under the app data dir, returning the absolute path.
/// File name: `<unix-millis>-<filename>`.
pub fn save_image(bytes: &[u8], filename: &str) -> Result<PathBuf, ImageError> {
    let id = format!("{}-{}", chrono::Utc::now().timestamp_millis(), filename);
    let dir = images_dir()?;
    let path = dir.join(id);
    fs::write(&path, bytes)?;
    Ok(path)
}

/// Delete an image file. Missing files are not an error.
pub fn delete_image(path: &Path) -> Result<(), ImageError> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e.into()),
    }
}

/// Remove and recreate the images directory.
pub fn delete_all_images() -> Result<(), ImageError> {
    let dir = images_dir()?;
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir)?;
    Ok(())
}

/// Copy the image at `path` onto the system clipboard as RGBA pixel data.
pub fn copy_image_to_clipboard(path: &Path) -> Result<(), ImageError> {
    let bytes = fs::read(path)?;
    let dyn_img = image::load_from_memory(&bytes).map_err(|e| ImageError::Decode(e.to_string()))?;
    let rgba = dyn_img.to_rgba8();
    let (w, h) = rgba.dimensions();
    let img_data = arboard::ImageData {
        width: w as usize,
        height: h as usize,
        bytes: Cow::Owned(rgba.into_raw()),
    };
    let mut clipboard =
        arboard::Clipboard::new().map_err(|e| ImageError::Clipboard(e.to_string()))?;
    clipboard
        .set_image(img_data)
        .map_err(|e| ImageError::Clipboard(e.to_string()))?;
    Ok(())
}
