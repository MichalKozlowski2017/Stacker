// Tauri requires a Windows entry point when not in debug mode.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

pub mod io;
pub mod stacking;

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Mutex;
use tauri::Emitter;

// ──────────────────────────────────────────────
// Shared app state
// ──────────────────────────────────────────────

#[derive(Default)]
pub struct StackerState {
    pub result: Mutex<Option<StackedImage>>,
}

pub struct StackedImage {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

// ──────────────────────────────────────────────
// DTO types
// ──────────────────────────────────────────────

#[derive(Serialize)]
pub struct ImageInfo {
    pub path: String,
    pub width: u32,
    pub height: u32,
    pub thumbnail: String,
}

#[derive(Serialize)]
pub struct StackResult {
    pub width: u32,
    pub height: u32,
    pub preview: String,
}

#[derive(Deserialize)]
pub struct StackOptions {
    pub align: bool,
    pub blend_radius: u32,
}

#[derive(Serialize, Clone)]
struct StackProgress {
    phase: String,
    current: u32,
    total: u32,
}

fn emit_progress(app: &tauri::AppHandle, phase: &str, current: u32, total: u32) {
    let _ = app.emit("stack-progress", StackProgress {
        phase: phase.to_string(),
        current,
        total,
    });
}

// ──────────────────────────────────────────────
// Tauri commands
// ──────────────────────────────────────────────

#[tauri::command]
async fn load_images(paths: Vec<String>) -> Result<Vec<ImageInfo>, String> {
    let mut results = Vec::new();

    for path_str in paths {
        let path = PathBuf::from(&path_str);
        // fast_info extracts EXIF/embedded-JPEG thumbnail without full decode
        let (width, height, thumb) =
            io::fast_info(&path).map_err(|e| format!("{}: {}", path_str, e))?;
        let thumb_b64 = io::encode_jpeg_base64(&thumb, 80).map_err(|e| e.to_string())?;

        results.push(ImageInfo {
            path: path_str,
            width,
            height,
            thumbnail: thumb_b64,
        });
    }

    Ok(results)
}

#[tauri::command]
async fn stack_images(
    app: tauri::AppHandle,
    paths: Vec<String>,
    options: StackOptions,
    state: tauri::State<'_, StackerState>,
) -> Result<StackResult, String> {
    let n = paths.len() as u32;

    // ── Phase 1: load full-resolution images ────────────────────────────────
    let mut images: Vec<image::RgbImage> = Vec::with_capacity(paths.len());
    for (i, p) in paths.iter().enumerate() {
        emit_progress(&app, "loading", (i + 1) as u32, n);
        let img = io::load_image(std::path::Path::new(p))
            .map_err(|e| format!("{}: {}", p, e))?;
        images.push(img.to_rgb8());
    }

    if images.is_empty() {
        return Err("No images provided".into());
    }

    // ── Phase 2: alignment ──────────────────────────────────────────────────
    if options.align && images.len() > 1 {
        let app2 = app.clone();
        images = stacking::align::align_stack(&images, move |cur, tot| {
            emit_progress(&app2, "aligning", cur, tot);
        })
        .map_err(|e| e.to_string())?;
    }

    // ── Phase 3: sharpness maps + blend ─────────────────────────────────────
    let app3 = app.clone();
    let stacked = stacking::blend::focus_stack(&images, options.blend_radius, move |cur, tot| {
        emit_progress(&app3, "sharpness", cur, tot);
    })
    .map_err(|e| e.to_string())?;

    let (width, height) = (stacked.width(), stacked.height());

    // ── Phase 4: encode preview ─────────────────────────────────────────────
    emit_progress(&app, "encoding", 0, 1);
    let preview_rgb = stacking::utils::resize_keep_aspect(&stacked, 1200);
    let preview_b64 = io::encode_png_base64(&preview_rgb).map_err(|e| e.to_string())?;
    emit_progress(&app, "encoding", 1, 1);

    let stacked_rgba = image::DynamicImage::ImageRgb8(stacked).to_rgba8();
    *state.result.lock().unwrap() = Some(StackedImage {
        width,
        height,
        rgba: stacked_rgba.into_raw(),
    });

    Ok(StackResult {
        width,
        height,
        preview: preview_b64,
    })
}

#[tauri::command]
async fn export_image(
    output_path: String,
    quality: u8,
    state: tauri::State<'_, StackerState>,
) -> Result<(), String> {
    let guard = state.result.lock().unwrap();
    let result = guard.as_ref().ok_or("No stacked image in memory")?;

    let rgba: image::RgbaImage =
        image::ImageBuffer::from_raw(result.width, result.height, result.rgba.clone())
            .ok_or("Failed to reconstruct image from buffer")?;

    io::export_image(
        &image::DynamicImage::ImageRgba8(rgba),
        &PathBuf::from(&output_path),
        quality,
    )
    .map_err(|e| e.to_string())
}

// ──────────────────────────────────────────────
// Entry point
// ──────────────────────────────────────────────

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_shell::init())
        .manage(StackerState::default())
        .invoke_handler(tauri::generate_handler![
            load_images,
            stack_images,
            export_image,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Stacker");
}
