/// Image alignment via phase correlation (FFT-based translation estimation).
///
/// All images in the stack are aligned to the first (reference) image.
/// Uses a 2D Hann window to suppress spectral leakage, and clamps detected
/// shifts to 5 % of image size to reject false peaks on natural images.
use anyhow::Result;
use image::{imageops, RgbImage};
use rustfft::{num_complex::Complex, FftPlanner};

// Maximum allowed shift as a fraction of image dimension.
// Focus stacking is almost always done on a tripod/rail, so shifts are small.
const MAX_SHIFT_FRAC: f32 = 0.05;

// ──────────────────────────────────────────────
// Public API
// ──────────────────────────────────────────────

/// Align all images in `stack` to the first image.
/// `on_progress(current, total)` is called after each image is aligned.
pub fn align_stack<F>(stack: &[RgbImage], on_progress: F) -> Result<Vec<RgbImage>>
where
    F: Fn(u32, u32),
{
    if stack.len() <= 1 {
        return Ok(stack.to_vec());
    }

    let reference = &stack[0];
    let (w, h) = reference.dimensions();

    // Downscale for faster correlation (≤ 512 px on the long side)
    let scale = (512.0_f32 / w.max(h) as f32).min(1.0);
    let sw = ((w as f32 * scale) as u32).max(1);
    let sh = ((h as f32 * scale) as u32).max(1);

    // Max allowed shift in the downscaled image
    let max_dx = (sw as f32 * MAX_SHIFT_FRAC).ceil() as i64;
    let max_dy = (sh as f32 * MAX_SHIFT_FRAC).ceil() as i64;

    let ref_small = imageops::resize(reference, sw, sh, imageops::FilterType::Triangle);
    let ref_luma  = rgb_to_luma_f32(&ref_small);
    let ref_win   = apply_hann(&ref_luma, sw as usize, sh as usize);

    let total = stack.len() as u32;
    let mut aligned = vec![reference.clone()];
    on_progress(1, total);

    for (i, img) in stack[1..].iter().enumerate() {
        let img_small = imageops::resize(img, sw, sh, imageops::FilterType::Triangle);
        let img_luma  = rgb_to_luma_f32(&img_small);
        let img_win   = apply_hann(&img_luma, sw as usize, sh as usize);

        let (dx_small, dy_small) =
            phase_correlation_2d(&ref_win, &img_win, sw as usize, sh as usize,
                                  max_dx, max_dy);

        // Map shift back to full-res coordinates
        let dx = (dx_small as f32 / scale).round() as i64;
        let dy = (dy_small as f32 / scale).round() as i64;

        aligned.push(translate(img, dx, dy, w, h));
        on_progress((i + 2) as u32, total);
    }

    Ok(aligned)
}

// ──────────────────────────────────────────────
// Phase correlation (proper 2D)
// ──────────────────────────────────────────────

/// Return `(dx, dy)` such that `b` ≈ `a` shifted by `(dx, dy)`.
/// Operates on pre-windowed float buffers.
/// Shifts larger than `max_dx` / `max_dy` are clamped to 0 (false peak).
fn phase_correlation_2d(
    a: &[f32], b: &[f32],
    w: usize, h: usize,
    max_dx: i64, max_dy: i64,
) -> (i64, i64) {
    let mut planner = FftPlanner::<f32>::new();
    let fft_row  = planner.plan_fft_forward(w);
    let fft_col  = planner.plan_fft_forward(h);
    let ifft_col = planner.plan_fft_inverse(h);
    let ifft_row = planner.plan_fft_inverse(w);

    let to_c = |buf: &[f32]| -> Vec<Complex<f32>> {
        buf.iter().map(|&v| Complex::new(v, 0.0)).collect()
    };

    let mut ac = to_c(a);
    let mut bc = to_c(b);

    // Forward 2D FFT: rows then columns
    let fft2d = |buf: &mut Vec<Complex<f32>>| {
        for row in buf.chunks_mut(w) { fft_row.process(row); }
        let mut col = vec![Complex::new(0.0f32, 0.0); h];
        for x in 0..w {
            for y in 0..h { col[y] = buf[y * w + x]; }
            fft_col.process(&mut col);
            for y in 0..h { buf[y * w + x] = col[y]; }
        }
    };
    fft2d(&mut ac);
    fft2d(&mut bc);

    // Normalised cross-power spectrum
    let mut cross: Vec<Complex<f32>> = ac.iter().zip(bc.iter()).map(|(a, b)| {
        let c = a * b.conj();
        let n = c.norm();
        if n > 1e-12 { c / n } else { Complex::new(0.0, 0.0) }
    }).collect();

    // Inverse 2D FFT: columns then rows
    let norm = 1.0 / (w * h) as f32;
    let mut col = vec![Complex::new(0.0f32, 0.0); h];
    for x in 0..w {
        for y in 0..h { col[y] = cross[y * w + x]; }
        ifft_col.process(&mut col);
        for y in 0..h { cross[y * w + x] = col[y] * norm; }
    }
    for row in cross.chunks_mut(w) { ifft_row.process(row); }

    // Find peak
    let (peak_idx, _) = cross.iter().enumerate()
        .max_by(|(_, a), (_, b)| a.re.partial_cmp(&b.re).unwrap())
        .unwrap();

    let raw_dx = (peak_idx % w) as i64;
    let raw_dy = (peak_idx / w) as i64;

    // Wrap: phase corr gives shifts in [0, N); >N/2 means negative
    let w2 = w as i64 / 2;
    let h2 = h as i64 / 2;
    let dx = if raw_dx > w2 { raw_dx - w as i64 } else { raw_dx };
    let dy = if raw_dy > h2 { raw_dy - h as i64 } else { raw_dy };

    // Reject if shift exceeds the cap (likely a false peak)
    let dx = if dx.abs() > max_dx { 0 } else { dx };
    let dy = if dy.abs() > max_dy { 0 } else { dy };

    (dx, dy)
}

// ──────────────────────────────────────────────
// Helpers
// ──────────────────────────────────────────────

/// Convert RGB to flat f32 luma [0..1].
fn rgb_to_luma_f32(img: &RgbImage) -> Vec<f32> {
    img.pixels()
        .map(|p| (0.299 * p[0] as f32 + 0.587 * p[1] as f32 + 0.114 * p[2] as f32) / 255.0)
        .collect()
}

/// Apply a separable 2D Hann window to reduce spectral leakage.
fn apply_hann(src: &[f32], w: usize, h: usize) -> Vec<f32> {
    use std::f32::consts::PI;
    let hann_x: Vec<f32> = (0..w).map(|i| 0.5 * (1.0 - (2.0 * PI * i as f32 / (w - 1) as f32).cos())).collect();
    let hann_y: Vec<f32> = (0..h).map(|i| 0.5 * (1.0 - (2.0 * PI * i as f32 / (h - 1) as f32).cos())).collect();
    src.iter().enumerate().map(|(idx, &v)| {
        v * hann_y[idx / w] * hann_x[idx % w]
    }).collect()
}

/// Translate `img` by `(dx, dy)` pixels on a canvas of `(out_w, out_h)`.
/// Pixels outside the source are black.
fn translate(img: &RgbImage, dx: i64, dy: i64, out_w: u32, out_h: u32) -> RgbImage {
    if dx == 0 && dy == 0 {
        return img.clone();
    }
    let mut out = RgbImage::new(out_w, out_h);
    let (src_w, src_h) = img.dimensions();
    let src_bytes = img.as_raw();

    for y in 0..out_h as i64 {
        let sy = y - dy;
        if sy < 0 || sy >= src_h as i64 { continue; }
        let dst_x0 = dx.max(0) as u32;
        let src_x0 = (-dx).max(0) as u32;
        let copy_w = (src_w.min(out_w) as i64 - dst_x0.max(src_x0) as i64
                       - (dx.abs() as i64 - 0).max(0)).max(0) as u32;
        // simpler: just copy pixel-by-pixel for correctness
        for x in 0..out_w as i64 {
            let sx = x - dx;
            if sx < 0 || sx >= src_w as i64 { continue; }
            let src_off = (sy as usize * src_w as usize + sx as usize) * 3;
            let dst_off = (y as usize * out_w as usize + x as usize) * 3;
            out.as_mut().get_mut(dst_off..dst_off + 3)
                .map(|d| d.copy_from_slice(&src_bytes[src_off..src_off + 3]));
        }
        let _ = copy_w; // suppress unused warning
    }
    out
}
