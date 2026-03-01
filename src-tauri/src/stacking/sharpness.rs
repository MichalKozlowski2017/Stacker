/// Per-pixel focus / sharpness measure based on the Laplacian operator.
///
/// Computed at half resolution for speed, then bilinearly upsampled back.
/// Weight maps are smoothed with a triple box-blur pass (≈ Gaussian) so
/// blending transitions look natural rather than blocky.
use image::{imageops, RgbImage};
use rayon::prelude::*;

/// Compute a flat `Vec<f32>` sharpness map, length = `w * h` (full-res).
/// Higher values = sharper focus.
pub fn sharpness_map_flat(img: &RgbImage, smooth_radius: u32) -> Vec<f32> {
    let (w, h) = img.dimensions();

    // ── half-resolution for speed ────────────────────────────────────────────
    let sw = (w / 2).max(1);
    let sh = (h / 2).max(1);
    let small = imageops::resize(img, sw, sh, imageops::FilterType::Triangle);

    let luma = rgb_to_luma_f32(&small);
    let sw_u = sw as usize;
    let sh_u = sh as usize;

    // ── Laplacian magnitude (5-tap) ──────────────────────────────────────────
    let luma_ref = luma.as_slice();
    let mut lap: Vec<f32> = (0..sw_u * sh_u)
        .into_par_iter()
        .map(|idx| {
            let y = idx / sw_u;
            let x = idx % sw_u;
            if x == 0 || x == sw_u - 1 || y == 0 || y == sh_u - 1 {
                return 0.0f32;
            }
            let c = luma_ref[y       * sw_u + x    ];
            let t = luma_ref[(y - 1) * sw_u + x    ];
            let b = luma_ref[(y + 1) * sw_u + x    ];
            let l = luma_ref[y       * sw_u + x - 1];
            let r = luma_ref[y       * sw_u + x + 1];
            let v = (t + b + l + r - 4.0 * c).abs();
            v * v   // squaring emphasises truly sharp regions
        })
        .collect();

    // ── Triple box-blur ≈ Gaussian (smooth weight maps, no banding) ──────────
    // Scale default radius to image size: ~1% of the longer dimension at
    // half-res.  For a 6048 px image: sw=3024, r≈30 → full-res equivalent
    // ~60 px.  This ensures the sharpness map is smooth enough that dark
    // / low-contrast areas never show random single-pixel frame choices.
    let default_r = ((sw.max(sh) as i32) / 100).clamp(8, 48);
    let r = if smooth_radius == 0 { default_r } else { smooth_radius.min(64) as i32 };
    for _ in 0..3 {
        lap = box_blur_sep(&lap, sw_u, sh_u, r);
    }

    // ── Bilinear upsample back to full resolution ────────────────────────────
    let sw_f = sw as f32;
    let sh_f = sh as f32;
    let w_u  = w as usize;
    let h_u  = h as usize;
    let lap_ref = lap.as_slice();

    (0..w_u * h_u)
        .into_par_iter()
        .map(|idx| {
            let py = (idx / w_u) as f32;
            let px = (idx % w_u) as f32;
            // Map to half-res coordinates (pixel centres)
            let sx = (px + 0.5) * sw_f / w as f32 - 0.5;
            let sy = (py + 0.5) * sh_f / h as f32 - 0.5;
            bilinear(lap_ref, sw_u, sh_u, sx, sy)
        })
        .collect()
}

// ──────────────────────────────────────────────
// Helpers
// ──────────────────────────────────────────────

fn rgb_to_luma_f32(img: &RgbImage) -> Vec<f32> {
    img.pixels()
        .map(|p| (0.299 * p[0] as f32 + 0.587 * p[1] as f32 + 0.114 * p[2] as f32) / 255.0)
        .collect()
}

/// Bilinear sample from a flat f32 buffer. Clamps coords to valid range.
fn bilinear(buf: &[f32], w: usize, h: usize, x: f32, y: f32) -> f32 {
    let x0 = (x.floor() as isize).clamp(0, w as isize - 1) as usize;
    let y0 = (y.floor() as isize).clamp(0, h as isize - 1) as usize;
    let x1 = (x0 + 1).min(w - 1);
    let y1 = (y0 + 1).min(h - 1);
    let fx = (x - x0 as f32).clamp(0.0, 1.0);
    let fy = (y - y0 as f32).clamp(0.0, 1.0);
    let v00 = buf[y0 * w + x0];
    let v10 = buf[y0 * w + x1];
    let v01 = buf[y1 * w + x0];
    let v11 = buf[y1 * w + x1];
    let top = v00 + (v10 - v00) * fx;
    let bot = v01 + (v11 - v01) * fx;
    top + (bot - top) * fy
}

/// Single-pass separable box blur (prefix-sum, O(w×h)).
fn box_blur_sep(src: &[f32], w: usize, h: usize, r: i32) -> Vec<f32> {
    // Horizontal
    let mut tmp = vec![0.0f32; w * h];
    for y in 0..h {
        let row = y * w;
        let mut psum = vec![0.0f32; w + 1];
        for x in 0..w { psum[x + 1] = psum[x] + src[row + x]; }
        for x in 0..w {
            let lo = (x as i32 - r).max(0) as usize;
            let hi = (x as i32 + r + 1).min(w as i32) as usize;
            tmp[row + x] = (psum[hi] - psum[lo]) / (hi - lo) as f32;
        }
    }
    // Vertical
    let mut out = vec![0.0f32; w * h];
    for x in 0..w {
        let mut psum = vec![0.0f32; h + 1];
        for y in 0..h { psum[y + 1] = psum[y] + tmp[y * w + x]; }
        for y in 0..h {
            let lo = (y as i32 - r).max(0) as usize;
            let hi = (y as i32 + r + 1).min(h as i32) as usize;
            out[y * w + x] = (psum[hi] - psum[lo]) / (hi - lo) as f32;
        }
    }
    out
}
