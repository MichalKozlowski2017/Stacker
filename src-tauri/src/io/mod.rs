pub mod raw;
pub mod standard;

use anyhow::{Context, Result};
use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
use image::{DynamicImage, GenericImageView, RgbImage};
use std::io::Cursor;
use std::path::Path;

// ──────────────────────────────────────────────
// Fast metadata + thumbnail (used by load_images)
// ──────────────────────────────────────────────

/// Fast image info: returns (width, height, thumbnail_rgb) without necessarily
/// decoding the full image.
///
/// Strategies (in order of preference):
///   1. JPEG          → extract embedded EXIF thumbnail (≈ instant)
///   2. RAW (CR2/NEF/ARW/DNG/…) → find largest embedded JPEG preview in the file
///   3. Everything else → full decode + downscale (slower, but rare in typical stacks)
pub fn fast_info(path: &Path) -> Result<(u32, u32, RgbImage)> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    let bytes = std::fs::read(path)
        .with_context(|| format!("Cannot read {}", path.display()))?;

    match ext.as_str() {
        // ── JPEG: try EXIF thumbnail first
        "jpg" | "jpeg" => {
            if let Some(thumb_bytes) = extract_jpeg_exif_thumbnail(&bytes) {
                if let Ok(img) = image::load_from_memory(&thumb_bytes) {
                    let (w, h) = full_jpeg_dimensions(&bytes)
                        .unwrap_or_else(|| (img.width(), img.height()));
                    let thumb = make_thumbnail_dyn(&img, 320);
                    return Ok((w, h, thumb));
                }
            }
            // Fall back: full decode
            let img = image::load_from_memory(&bytes).context("JPEG decode failed")?;
            let (w, h) = (img.width(), img.height());
            Ok((w, h, make_thumbnail_dyn(&img, 320)))
        }

        // ── RAW formats: find the largest embedded JPEG in the file
        "cr2" | "cr3" | "nef" | "nrw" | "arw" | "srf" | "sr2" | "orf" | "rw2" | "pef"
        | "dng" | "raf" | "dcr" | "kdc" | "mrw" | "x3f" | "raw" | "rwl" => {
            if let Some(jpeg_bytes) = find_largest_embedded_jpeg(&bytes) {
                if let Ok(img) = image::load_from_memory(jpeg_bytes) {
                    // rawloader knows actual sensor dimensions; use it for reporting only
                    let (w, h) = rawloader_dimensions(path)
                        .unwrap_or_else(|| (img.width(), img.height()));
                    let thumb = make_thumbnail_dyn(&img, 320);
                    return Ok((w, h, thumb));
                }
            }
            // Fall back: full RAW decode (slow, but accurate)
            let img = raw::load_raw(path).context("RAW decode failed")?;
            let (w, h) = (img.width(), img.height());
            Ok((w, h, make_thumbnail_dyn(&img, 320)))
        }

        // ── Everything else: full decode
        _ => {
            let img = image::load_from_memory(&bytes).context("Image decode failed")?;
            let (w, h) = (img.width(), img.height());
            Ok((w, h, make_thumbnail_dyn(&img, 320)))
        }
    }
}

// ──────────────────────────────────────────────
// JPEG EXIF thumbnail extractor (no extra crate)
// ──────────────────────────────────────────────

/// Extract the embedded JPEG thumbnail from an EXIF APP1 segment.
/// Returns `None` if not found or invalid.
fn extract_jpeg_exif_thumbnail(data: &[u8]) -> Option<Vec<u8>> {
    if data.len() < 4 || data[0] != 0xFF || data[1] != 0xD8 {
        return None;
    }
    let mut pos = 2usize;
    while pos + 4 <= data.len() {
        if data[pos] != 0xFF {
            return None;
        }
        let marker = data[pos + 1];
        if pos + 4 > data.len() {
            return None;
        }
        let seg_len = u16::from_be_bytes([data[pos + 2], data[pos + 3]]) as usize;
        // APP1 with "Exif\0\0" header
        if marker == 0xE1
            && pos + 10 <= data.len()
            && &data[pos + 4..pos + 10] == b"Exif\0\0"
        {
            let exif_data = &data[pos + 10..pos + 2 + seg_len];
            if let Some(thumb) = parse_exif_thumbnail(exif_data) {
                return Some(thumb);
            }
        }
        if seg_len < 2 { return None; }
        pos += 2 + seg_len;
        // SOS = start of scan; no more headers after this
        if marker == 0xDA { break; }
    }
    None
}

/// Parse a TIFF-structured EXIF block and extract the IFD1 thumbnail JPEG.
fn parse_exif_thumbnail(data: &[u8]) -> Option<Vec<u8>> {
    if data.len() < 8 { return None; }
    let le = &data[0..2] == b"II";
    let r16 = |off: usize| -> Option<u32> {
        let b = data.get(off..off + 2)?;
        Some(if le { u16::from_le_bytes([b[0], b[1]]) as u32 }
             else  { u16::from_be_bytes([b[0], b[1]]) as u32 })
    };
    let r32 = |off: usize| -> Option<u32> {
        let b = data.get(off..off + 4)?;
        Some(if le { u32::from_le_bytes([b[0], b[1], b[2], b[3]]) }
             else  { u32::from_be_bytes([b[0], b[1], b[2], b[3]]) })
    };
    if r16(2)? != 42 { return None; }        // TIFF magic
    let ifd0_off = r32(4)? as usize;
    let n0 = r16(ifd0_off)? as usize;
    let ifd1_off = r32(ifd0_off + 2 + n0 * 12)? as usize;
    if ifd1_off == 0 { return None; }
    let n1 = r16(ifd1_off)? as usize;

    let mut thumb_off: Option<u32> = None;
    let mut thumb_len: Option<u32> = None;
    for i in 0..n1 {
        let e = ifd1_off + 2 + i * 12;
        match r16(e)? {
            0x0201 => thumb_off = Some(r32(e + 8)?),
            0x0202 => thumb_len = Some(r32(e + 8)?),
            _ => {}
        }
    }
    let off = thumb_off? as usize;
    let len = thumb_len? as usize;
    data.get(off..off + len).map(|s| s.to_vec())
}

/// Read actual JPEG dimensions from SOF header (no full decode).
fn full_jpeg_dimensions(data: &[u8]) -> Option<(u32, u32)> {
    if data.len() < 4 || data[0] != 0xFF || data[1] != 0xD8 { return None; }
    let mut pos = 2usize;
    while pos + 4 <= data.len() {
        if data[pos] != 0xFF { return None; }
        let marker = data[pos + 1];
        let seg_len = u16::from_be_bytes([data[pos + 2], data[pos + 3]]) as usize;
        // SOF markers: C0-CF (except C4, C8, CC)
        if matches!(marker, 0xC0 | 0xC1 | 0xC2 | 0xC3 | 0xC5 | 0xC6 | 0xC7 | 0xC9 | 0xCA | 0xCB | 0xCD | 0xCE | 0xCF)
            && pos + 9 <= data.len()
        {
            let h = u16::from_be_bytes([data[pos + 5], data[pos + 6]]) as u32;
            let w = u16::from_be_bytes([data[pos + 7], data[pos + 8]]) as u32;
            return Some((w, h));
        }
        if seg_len < 2 { return None; }
        pos += 2 + seg_len;
    }
    None
}

// ──────────────────────────────────────────────
// Embedded JPEG finder (for RAW files)
// ──────────────────────────────────────────────

/// Find the LARGEST embedded JPEG in a file (e.g. full-size preview in CR2/NEF/ARW).
/// Returns a slice of the embedded JPEG bytes.
fn find_largest_embedded_jpeg(data: &[u8]) -> Option<&[u8]> {
    const SOI: [u8; 3] = [0xFF, 0xD8, 0xFF];
    const EOI: [u8; 2] = [0xFF, 0xD9];

    let mut best: Option<&[u8]> = None;

    for (start, _) in data.windows(3).enumerate().filter(|(_, w)| *w == &SOI) {
        // Find the corresponding EOI from this position
        if let Some(rel_end) = data[start..].windows(2).rposition(|w| w == EOI) {
            let end = start + rel_end + 2;
            let jpeg = &data[start..end];
            // Must be at least 64 KB to be a real preview (not the tiny 160×120 thumb)
            if jpeg.len() >= 65_536 {
                if best.map_or(true, |b: &[u8]| jpeg.len() > b.len()) {
                    best = Some(jpeg);
                }
            }
        }
    }
    best
}

/// Ask rawloader for sensor dimensions without full demosaicing.
fn rawloader_dimensions(path: &Path) -> Option<(u32, u32)> {
    rawloader::decode_file(path)
        .ok()
        .map(|r| (r.width as u32, r.height as u32))
}

// ──────────────────────────────────────────────
// Full-quality load (used by stack_images)
// ──────────────────────────────────────────────

/// Load any supported image at full quality for stacking.
pub fn load_image(path: &Path) -> Result<DynamicImage> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    match ext.as_str() {
        "cr2" | "cr3" | "nef" | "nrw" | "arw" | "srf" | "sr2" | "orf" | "rw2" | "pef"
        | "dng" | "raf" | "dcr" | "kdc" | "mrw" | "x3f" | "raw" | "rwl" => {
            raw::load_raw(path).context("Failed to decode RAW image")
        }
        _ => standard::load_standard(path).context("Failed to decode image"),
    }
}

// ──────────────────────────────────────────────
// Shared helpers
// ──────────────────────────────────────────────

fn make_thumbnail_dyn(img: &DynamicImage, max_dim: u32) -> RgbImage {
    let (w, h) = img.dimensions();
    let scale = (max_dim as f32 / w.max(h) as f32).min(1.0);
    let nw = ((w as f32 * scale) as u32).max(1);
    let nh = ((h as f32 * scale) as u32).max(1);
    img.resize(nw, nh, image::imageops::FilterType::Triangle).to_rgb8()
}

/// Create a thumbnail with the long edge capped at `max_dim`.
pub fn make_thumbnail(img: &DynamicImage, max_dim: u32) -> RgbImage {
    make_thumbnail_dyn(img, max_dim)
}

/// Encode an `RgbImage` as a JPEG and return as base64 string.
pub fn encode_jpeg_base64(img: &RgbImage, quality: u8) -> Result<String> {
    let mut buf = Vec::new();
    let mut cursor = Cursor::new(&mut buf);
    let mut encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut cursor, quality);
    encoder
        .encode(img.as_raw(), img.width(), img.height(), image::ColorType::Rgb8.into())
        .context("JPEG encoding failed")?;
    Ok(B64.encode(&buf))
}

/// Encode an `RgbImage` as a PNG and return as base64 string.
pub fn encode_png_base64(img: &RgbImage) -> Result<String> {
    let dyn_img = DynamicImage::ImageRgb8(img.clone());
    let mut buf = Vec::new();
    let mut cursor = Cursor::new(&mut buf);
    dyn_img
        .write_to(&mut cursor, image::ImageFormat::Png)
        .context("PNG encoding failed")?;
    Ok(B64.encode(&buf))
}

/// Save a `DynamicImage` to disk; format is determined by the file extension.
pub fn export_image(img: &DynamicImage, path: &Path, jpeg_quality: u8) -> Result<()> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("png")
        .to_lowercase();

    match ext.as_str() {
        "jpg" | "jpeg" => {
            let file = std::fs::File::create(path).context("Cannot create file")?;
            let mut encoder =
                image::codecs::jpeg::JpegEncoder::new_with_quality(file, jpeg_quality);
            let rgb = img.to_rgb8();
            encoder
                .encode(rgb.as_raw(), rgb.width(), rgb.height(), image::ColorType::Rgb8.into())
                .context("JPEG export failed")?;
        }
        "tif" | "tiff" => {
            img.save(path).context("TIFF export failed")?;
        }
        _ => {
            img.save(path).context("PNG export failed")?;
        }
    }
    Ok(())
}
