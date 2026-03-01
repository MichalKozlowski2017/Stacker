/// Focus stacking: blend a stack of images into one all-sharp result.
///
/// Method: Soft sharpness-weighted blend (Helicon "Method A" style)
/// -----------------------------------------------------------------
/// For each pixel the output colour is a weighted average of all source
/// frames.  The weight for frame k at pixel p is:
///
///     w[k][p] = sharpness[k][p] ^ POWER
///
/// Key properties
/// ──────────────
/// • Dark / textureless areas (sharpness ≈ 0 in every frame):
///   All weights are equal → simple average of all frames → smooth,
///   natural colour with no artefacts or "broken" look.
///
/// • Well-focused areas (one frame clearly sharper than others):
///   With POWER = 10 the sharpest frame contributes ~99 % of the weight.
///   Focus is preserved almost as well as a hard argmax — but without
///   the hard depth-map cuts that create visible seams.
///
/// • No ghosting: in the sharp→blurry transition the blurry frame's
///   weight is tiny, so the weighted average is almost identical to the
///   sharpest source frame.
use anyhow::Result;
use image::RgbImage;
use rayon::prelude::*;
use std::sync::atomic::{AtomicU32, Ordering};

use super::sharpness;

/// Exponent applied to NORMALISED sharpness values before weighting.
/// Applied after dividing each pixel's per-frame sharpness by the
/// per-pixel maximum, so the range is always [0..1] regardless of
/// absolute brightness.  Value 6 gives clean focus selection while
/// keeping dark / low-contrast areas smooth.
const POWER: i32 = 6;

pub fn focus_stack<F>(stack: &[RgbImage], blend_radius: u32, on_sharpness: F) -> Result<RgbImage>
where
    F: Fn(u32, u32) + Sync,
{
    if stack.is_empty() {
        return Err(anyhow::anyhow!("Empty image stack"));
    }
    if stack.len() == 1 {
        return Ok(stack[0].clone());
    }

    let (w, h) = stack[0].dimensions();
    for (i, img) in stack.iter().enumerate() {
        if img.dimensions() != (w, h) {
            return Err(anyhow::anyhow!(
                "Image {} has size {:?}; expected {:?}",
                i, img.dimensions(), (w, h)
            ));
        }
    }

    let n   = stack.len();
    let npx = (w * h) as usize;

    // ── Step 1: sharpness maps (parallel across images) ───────────────────────
    let total_maps = n as u32;
    let done_maps  = AtomicU32::new(0);
    let maps: Vec<Vec<f32>> = stack
        .par_iter()
        .map(|img| {
            let map = sharpness::sharpness_map_flat(img, blend_radius);
            let c = done_maps.fetch_add(1, Ordering::Relaxed) + 1;
            on_sharpness(c, total_maps);
            map
        })
        .collect();

    let raw: Vec<&[u8]> = stack.iter().map(|img| img.as_raw().as_slice()).collect();

    // ── Step 2: image-wide sharpness mean (absolute threshold reference) ──────
    // We need an absolute floor: pixels where every frame has near-zero
    // sharpness (deep-shadow gaps, uniform surfaces) must use a plain average
    // regardless of which frame "won" the normalised competition.
    // Using 2 % of the image mean as the cutoff.
    let global_mean: f32 = {
        let sum: f64 = maps.iter()
            .flat_map(|m| m.iter())
            .map(|&v| v as f64)
            .sum();
        (sum / (maps.len() * npx) as f64) as f32
    };
    // 8 % of global mean: captures dark gaps and low-contrast surfaces.
    // Below this threshold we use the middle frame directly (no blending).
    // Between threshold and 2× threshold we interpolate smoothly to avoid
    // a visible hard seam at the boundary.
    let abs_threshold = global_mean * 0.08;

    // ── Step 3: normalised soft-weighted blend ────────────────────────────────
    //
    // For each pixel p, find the per-pixel maximum sharpness across all frames:
    //   sharp_max[p] = max_k(sharp[k][p])
    //
    // Then normalise each frame's sharpness:
    //   s_norm[k][p] = sharp[k][p] / (sharp_max[p] + ε)
    //
    // Weight:
    //   w[k][p] = s_norm[k][p] ^ POWER
    //
    // This fixes dark / textureless areas completely:
    //   • All frames have sharp ≈ 0  →  all s_norm ≈ 1  →  equal weights
    //     →  simple average  →  natural colour, zero artefacts.
    //
    // In well-focused areas one frame has s_norm = 1.0, others ≪ 1.
    // With POWER = 6 that one frame dominates (weight ratio ~10^6:1).
    let out_raw: Vec<u8> = (0..npx)
        .into_par_iter()
        .flat_map_iter(|idx| {
            let base = idx * 3;

            // Per-pixel max sharpness
            let sharp_max = (0..n)
                .map(|k| maps[k][idx])
                .fold(0.0f32, f32::max);

            // ── Darkness handling ─────────────────────────────────────────────
            // Below abs_threshold: use middle frame verbatim (no blending).
            //   Reason: averaging misaligned frames creates blurry ghosts in
            //   dark gaps; middle frame is the cleanest single-source rendition.
            //
            // Between abs_threshold and 2×abs_threshold: smooth crossfade
            //   between middle frame and weighted blend to avoid a hard seam.
            let mid = n / 2;
            if sharp_max < abs_threshold {
                let p = &raw[mid][base..base + 3];
                return [p[0], p[1], p[2]];
            }
            // Blend factor: 0.0 at abs_threshold → 1.0 at 2×abs_threshold
            let blend_alpha = ((sharp_max - abs_threshold) / abs_threshold).min(1.0) as f64;

            // Epsilon: 1% of the image-wide mean sharpness keeps
            // normalisation stable even when sharp_max ≈ 0.
            let eps = (sharp_max * 0.01).max(1e-8);
            let denom = sharp_max + eps;

            let mut weight_sum = 0.0f64;
            let mut r = 0.0f64;
            let mut g = 0.0f64;
            let mut b = 0.0f64;

            for k in 0..n {
                let s_norm = (maps[k][idx] / denom) as f64;  // in [0, 1]
                let w = s_norm.powi(POWER);
                let p = &raw[k][base..base + 3];
                r += p[0] as f64 * w;
                g += p[1] as f64 * w;
                b += p[2] as f64 * w;
                weight_sum += w;
            }

            // weight_sum is always > 0 after normalisation
            let inv = 1.0 / weight_sum;
            let br = (r * inv).clamp(0.0, 255.0);
            let bg = (g * inv).clamp(0.0, 255.0);
            let bb = (b * inv).clamp(0.0, 255.0);

            // Smooth crossfade with middle frame in the transition zone
            let mp = &raw[mid][base..base + 3];
            let mr = mp[0] as f64;
            let mg = mp[1] as f64;
            let mb = mp[2] as f64;
            [
                (mr + (br - mr) * blend_alpha).round() as u8,
                (mg + (bg - mg) * blend_alpha).round() as u8,
                (mb + (bb - mb) * blend_alpha).round() as u8,
            ]
        })
        .collect();

    RgbImage::from_raw(w, h, out_raw)
        .ok_or_else(|| anyhow::anyhow!("Failed to construct output image"))
}
