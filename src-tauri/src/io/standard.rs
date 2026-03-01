use anyhow::{Context, Result};
use image::DynamicImage;
use std::path::Path;

/// Decode a standard image (JPEG, PNG, TIFF, BMP, …) using the `image` crate.
pub fn load_standard(path: &Path) -> Result<DynamicImage> {
    image::open(path).with_context(|| format!("Cannot open image: {}", path.display()))
}
