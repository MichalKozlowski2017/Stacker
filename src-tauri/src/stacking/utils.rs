use image::{imageops, RgbImage};

/// Resize `img` so the longest edge is at most `max_dim` pixels, preserving
/// aspect ratio.  Returns the original if it's already smaller.
pub fn resize_keep_aspect(img: &RgbImage, max_dim: u32) -> RgbImage {
    let (w, h) = img.dimensions();
    if w <= max_dim && h <= max_dim {
        return img.clone();
    }
    let scale = max_dim as f32 / w.max(h) as f32;
    let nw = ((w as f32 * scale) as u32).max(1);
    let nh = ((h as f32 * scale) as u32).max(1);
    imageops::resize(img, nw, nh, imageops::FilterType::Lanczos3)
}
