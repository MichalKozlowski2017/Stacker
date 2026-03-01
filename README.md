# Stacker

**Focus stacking app for macro photography.** Drop a set of focus-bracketed frames and get a single, fully sharp image — no Photoshop required.

Built with [Tauri 2](https://tauri.app), Rust, and Svelte 5. Runs natively on macOS.

---

## Features

- **RAW support** — reads Canon CR2/CR3, Nikon NEF, Sony ARW and other common RAW formats alongside JPEG/PNG/TIFF
- **Fast thumbnail loading** — extracts embedded JPEG previews so even 50 MB RAW files load instantly
- **Auto-alignment** — optional phase-correlation FFT alignment corrects small shifts between frames (handy for rail sequences without live-view lock)
- **Smart blending** — per-pixel Laplacian sharpness maps select the sharpest content from each frame; dark/uniform areas fall back to the middle frame to avoid ghosting
- **Real-time progress** — live status bar shows which phase is running (loading → aligning → sharpness → encoding) with a per-frame counter and elapsed timer
- **Flexible export** — save the result as **PNG** (lossless), **JPEG** (with adjustable quality), or **TIFF**

---

## How to use

### 1. Add frames

Drag and drop your focus-bracketed images onto the app window, or click the drop zone to open a file picker. You can add files in multiple batches — duplicates are ignored automatically.

> **Tip:** Order doesn't matter. Stacker picks the sharpest pixel from each frame regardless of sequence.

### 2. Configure options

| Option | Default | Description |
|---|---|---|
| **Align frames** | Off | Corrects small X/Y shifts between frames using phase correlation. Turn on if you shot handheld or your rail shifts the camera slightly. Adds a few seconds for large stacks. |
| **Blend radius** | 16 | Controls how aggressively the sharpness map is smoothed. Higher = softer transitions between in-focus regions. Lower = more local, detailed selection. |

### 3. Stack

Click **Stack**. The status bar shows progress in real time. For a 13-frame 6048×4024 RAW sequence it typically takes 10–30 seconds on Apple Silicon.

### 4. Export

Once stacking is complete, a preview appears on the right. Choose your output format:

- **PNG** — lossless, best for further editing
- **JPG** — smaller file; drag the quality slider (50–100%)
- **TIFF** — lossless, compatible with Lightroom / Photoshop workflows

Click **Export** and choose where to save the file.

---

## Building from source

### Requirements

- [Rust](https://rustup.rs) (stable toolchain)
- [Node.js](https://nodejs.org) ≥ 18
- macOS with Xcode Command Line Tools

```bash
# Install dependencies
npm install

# Development (hot-reload)
source "$HOME/.cargo/env"
npx tauri dev

# Production build
npx tauri build
```

> **macOS SDK note:** If you're on macOS 15+ and see linker errors about undefined symbols, make sure `.cargo/config.toml` at the project root sets `MACOSX_DEPLOYMENT_TARGET` to match your SDK. The repo already includes the correct value.

---

## Tech stack

| Layer | Technology |
|---|---|
| Frontend | Svelte 5 + TypeScript |
| Backend | Rust (via Tauri 2) |
| Image decoding | `image` crate + `rawloader` |
| Sharpness maps | Laplacian filter + triple box blur |
| Alignment | 2D FFT phase correlation (`rustfft`) |
| Parallelism | `rayon` |
| Packaging | Tauri (native macOS `.app`) |

---

## License

MIT
