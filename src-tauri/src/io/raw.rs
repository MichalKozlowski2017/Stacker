use anyhow::{Context, Result};
use image::{DynamicImage, Rgb, RgbImage};
use std::path::Path;

/// Decode a RAW file using the `rawloader` crate and return an `RgbImage`.
///
/// The output is a 16-bit-per-channel linear image converted to 8-bit sRGB
/// after a basic gamma curve application (γ ≈ 1/2.2).  A full pipeline
/// (white balance, demosaicing quality, tone mapping) can be added later.
pub fn load_raw(path: &Path) -> Result<DynamicImage> {
    let raw = rawloader::decode_file(path).context("rawloader failed to decode file")?;

    // rawloader returns CFA (Bayer pattern) data as f32 or u16.
    // We use the built-in simple demosaic from rawloader's `imagepipe` / we do it manually.
    match raw.data {
        rawloader::RawImageData::Float(ref data) => {
            let img = float_to_rgb8(&raw, data);
            Ok(DynamicImage::ImageRgb8(img))
        }
        rawloader::RawImageData::Integer(ref data) => {
            // Normalize u16 to f32 then process
            let max_val = raw.whitelevels[0] as f32;
            let float_data: Vec<f32> = data.iter().map(|&v| v as f32 / max_val).collect();
            let img = float_to_rgb8(&raw, &float_data);
            Ok(DynamicImage::ImageRgb8(img))
        }
    }
}

// ──────────────────────────────────────────────
// Internal helpers
// ──────────────────────────────────────────────

/// Convert flat CFA (Bayer) f32 data to an 8-bit sRGB `RgbImage`.
///
/// Uses a simple nearest-neighbour / bilinear 2×2 Bayer demosaic.
fn float_to_rgb8(raw: &rawloader::RawImage, data: &[f32]) -> RgbImage {
    let width = raw.width as u32;
    let height = raw.height as u32;

    // White-balance multipliers (normalised so the green channel = 1.0)
    let wb = normalise_wb(&raw.wb_coeffs);

    let mut out = RgbImage::new(width, height);

    // Determine Bayer pattern layout
    // rawloader exposes the CFA pattern as a string like "RGGB", "BGGR", etc.
    let pattern = bayer_offsets(&raw.cfa);

    for y in 0..height {
        for x in 0..width {
            let color = bayer_channel(x, y, &pattern);
            let idx = (y * width + x) as usize;
            let v = (data[idx] * wb[color]).clamp(0.0, 1.0);

            // For a single pixel we snap to the nearest 2×2 super-pixel.
            // A real bilinear demosaic fills all three channels per pixel.
            // This quick path assigns each sensor pixel to one channel and
            // copies from neighbours for the missing channels – good enough
            // for alignment and stacking focus detection.
            let px = out.get_pixel_mut(x, y);
            match color {
                0 => px[0] = (apply_gamma(v) * 255.0) as u8,     // R
                1 | 3 => px[1] = (apply_gamma(v) * 255.0) as u8, // G (Gr/Gb)
                2 => px[2] = (apply_gamma(v) * 255.0) as u8,     // B
                _ => {}
            }
        }
    }

    // Bilinear fill of missing channels (fast approximate pass)
    fill_missing_channels(&mut out, &pattern);

    out
}

/// Apply sRGB gamma (1/2.2 approximation).
#[inline]
fn apply_gamma(v: f32) -> f32 {
    v.powf(1.0 / 2.2)
}

/// Normalise white-balance coefficients so the green channel multiplier = 1.0.
fn normalise_wb(wb: &[f32; 4]) -> [f32; 4] {
    let g = (wb[1] + wb[3]) / 2.0;
    if g == 0.0 {
        return [1.0, 1.0, 1.0, 1.0];
    }
    [wb[0] / g, 1.0, wb[2] / g, 1.0]
}

/// Returns (r_dx, r_dy, b_dx, b_dy) offsets within a 2×2 Bayer cell.
/// Channels: 0 = R, 1 = Gr, 2 = B, 3 = Gb
fn bayer_offsets(cfa: &rawloader::CFA) -> [[usize; 4]; 2] {
    // Build a 2×2 channel map from the CFA pattern
    let mut map = [[0usize; 4]; 2]; // [row 0..1][col 0..3, but we use 0..2]
    // Actually we need a 2×2 grid → flat 4-entry array [row0col0, row0col1, row1col0, row1col1]
    let mut grid = [0usize; 4];
    for row in 0..2usize {
        for col in 0..2usize {
            grid[row * 2 + col] = match cfa.color_at(row, col) {
                0 => 0, // R
                1 => 1, // G at row-even
                2 => 2, // B
                3 => 3, // G at row-odd
                _ => 1,
            };
        }
    }
    // Reuse map[0] as the grid (row-major)
    map[0] = [grid[0], grid[1], grid[2], grid[3]];
    map
}

/// Return the Bayer channel (0=R,1=Gr,2=B,3=Gb) for pixel (x,y).
#[inline]
fn bayer_channel(x: u32, y: u32, offsets: &[[usize; 4]; 2]) -> usize {
    offsets[0][(y as usize % 2) * 2 + (x as usize % 2)]
}

/// Quick bilinear fill: for each pixel, copy channels from neighbours that
/// were captured in the same Bayer cell.
fn fill_missing_channels(img: &mut RgbImage, pattern: &[[usize; 4]; 2]) {
    let (w, h) = img.dimensions();

    // Work in 2×2 super-pixels
    for y in (0..h.saturating_sub(1)).step_by(2) {
        for x in (0..w.saturating_sub(1)).step_by(2) {
            // Gather the four colour values in this super-pixel
            let mut vals = [0u8; 4]; // indexed by channel 0-3
            for dy in 0..2u32 {
                for dx in 0..2u32 {
                    let ch = bayer_channel(x + dx, y + dy, pattern);
                    let px = img.get_pixel(x + dx, y + dy);
                    vals[ch] = match ch {
                        0 => px[0],
                        1 | 3 => px[1],
                        2 => px[2],
                        _ => 0,
                    };
                }
            }
            // Write all channels into all four pixels of the super-pixel
            let [r, gr, b, gb] = [vals[0], vals[1], vals[2], vals[3]];
            let g_avg = ((gr as u16 + gb as u16) / 2) as u8;
            for dy in 0..2u32 {
                for dx in 0..2u32 {
                    let px = img.get_pixel_mut(x + dx, y + dy);
                    *px = Rgb([r, g_avg, b]);
                }
            }
        }
    }
}
