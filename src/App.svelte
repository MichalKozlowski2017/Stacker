<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { listen, type UnlistenFn } from "@tauri-apps/api/event";
  import DropZone from "./components/DropZone.svelte";
  import ImageList from "./components/ImageList.svelte";
  import Preview from "./components/Preview.svelte";
  import ExportPanel from "./components/ExportPanel.svelte";
  import StatusBar from "./components/StatusBar.svelte";
  import type { ImageInfo, StackOptions, StackResult, Status } from "./types";

  // ──────────────────────────────────────────────
  // State
  // ──────────────────────────────────────────────
  let images = $state<ImageInfo[]>([]);
  let result = $state<StackResult | null>(null);
  let status = $state<Status>("idle");
  let errorMsg = $state<string | null>(null);
  let lastExport = $state<string | null>(null);

  let options = $state<StackOptions>({ align: false, blend_radius: 16 });

  // Loading progress (per-file)
  let loadCurrent = $state(0);
  let loadTotal = $state(0);
  let loadFile = $state("");

  // Stacking timer
  let stackElapsed = $state(0);
  let _timerInterval: ReturnType<typeof setInterval> | null = null;

  // Stacking progress (from Rust events)
  let stackPhase = $state("");
  let stackCurrent = $state(0);
  let stackTotal = $state(0);

  interface StackProgressPayload { phase: string; current: number; total: number; }

  function startTimer() {
    stackElapsed = 0;
    _timerInterval = setInterval(() => { stackElapsed += 1; }, 1000);
  }
  function stopTimer() {
    if (_timerInterval) { clearInterval(_timerInterval); _timerInterval = null; }
  }

  // ──────────────────────────────────────────────
  // Actions
  // ──────────────────────────────────────────────
  async function handleFiles(paths: string[]) {
    status = "loading";
    errorMsg = null;
    loadCurrent = 0;
    loadTotal = paths.length;

    const newImages: ImageInfo[] = [];
    const existingPaths = new Set(images.map((i) => i.path));

    for (const path of paths) {
      loadFile = path.split("/").at(-1) ?? path;
      loadCurrent += 1;
      if (existingPaths.has(path)) continue;
      try {
        const [info]: ImageInfo[] = await invoke("load_images", { paths: [path] });
        newImages.push(info);
      } catch (e: any) {
        errorMsg = String(e);
        status = "error";
        return;
      }
    }

    images = [...images, ...newImages];
    status = "idle";
  }

  async function runStack() {
    if (images.length < 2) return;
    status = "stacking";
    errorMsg = null;
    result = null;
    stackPhase = "";
    stackCurrent = 0;
    stackTotal = 0;
    startTimer();

    const unlisten: UnlistenFn = await listen<StackProgressPayload>("stack-progress", (ev) => {
      stackPhase = ev.payload.phase;
      stackCurrent = ev.payload.current;
      stackTotal = ev.payload.total;
    });

    try {
      const res: StackResult = await invoke("stack_images", {
        paths: images.map((i) => i.path),
        options,
      });
      result = res;
      status = "done";
    } catch (e: any) {
      errorMsg = String(e);
      status = "error";
    } finally {
      unlisten();
      stopTimer();
    }
  }

  function removeImage(idx: number) {
    images = images.filter((_, i) => i !== idx);
    if (images.length === 0) { result = null; status = "idle"; }
  }

  function clearImages() {
    images = [];
    result = null;
    status = "idle";
    errorMsg = null;
  }
</script>

<!-- ──────────────────────────────────────────── -->
<!-- Layout                                       -->
<!-- ──────────────────────────────────────────── -->
<div class="layout">

  <div class="content">
    <!-- ── LEFT SIDEBAR ──────────────────────────────────── -->
    <aside class="sidebar">
    <header class="app-header">
      <span class="logo">◈</span>
      <span class="app-name">Stacker</span>
    </header>

    <DropZone onFiles={handleFiles} />

    {#if images.length > 0}
      <ImageList {images} onRemove={removeImage} onClear={clearImages} />
    {/if}

    <!-- Stack Options -->
    <section class="section">
      <h3 class="section-title">Opcje</h3>
      <label class="option-row">
        <input type="checkbox" bind:checked={options.align} />
        <span>Wyrównaj zdjęcia (alignment)</span>
      </label>

      <div class="option-row col">
        <span>Promień wygładzania: <strong>{options.blend_radius}px</strong></span>
        <input
          type="range"
          min="0"
          max="64"
          step="1"
          bind:value={options.blend_radius}
        />
      </div>
    </section>

    <!-- Stack Button -->
    <button
      class="btn-primary stack-btn"
      disabled={images.length < 2 || status === "stacking" || status === "loading"}
      onclick={runStack}
    >
      {#if status === "stacking"}
        <span class="mini-spin"></span> Stackowanie…
      {:else}
        ▶ Połącz {images.length} {images.length === 1 ? "zdjęcie" : images.length < 5 ? "zdjęcia" : "zdjęć"}
      {/if}
    </button>

    {#if errorMsg}
      <div class="error-box">{errorMsg}</div>
    {/if}

    <!-- Export -->
    {#if result}
      <section class="section">
        <h3 class="section-title">Eksport</h3>
        <ExportPanel
          hasResult={!!result}
          onExported={(p) => { lastExport = p; }}
        />
        {#if lastExport}
          <p class="success-msg">✓ Zapisano: {lastExport.split("/").at(-1)}</p>
        {/if}
      </section>
    {/if}
  </aside>

    <!-- ── MAIN AREA ──────────────────────────────────── -->
    <main class="main">
      <Preview
        preview={result?.preview ?? null}
        width={result?.width ?? 0}
        height={result?.height ?? 0}
        loading={status === "stacking"}
      />
    </main>
  </div>

  <!-- ── STATUS BAR ──────────────────────────────────── -->
  <StatusBar
    phase={status}
    {loadCurrent}
    {loadTotal}
    {loadFile}
    {stackElapsed}
    {stackPhase}
    {stackCurrent}
    {stackTotal}
    resultWidth={result?.width ?? 0}
    resultHeight={result?.height ?? 0}
    errorMsg={errorMsg ?? ""}
  />

</div>

<style>
  .layout {
    display: flex;
    flex-direction: column;
    height: 100vh;
    overflow: hidden;
  }

  .content {
    display: flex;
    flex: 1;
    min-height: 0;
  }

  /* ── Sidebar ── */
  .sidebar {
    width: 280px;
    min-width: 240px;
    max-width: 300px;
    background: var(--surface);
    border-right: 1px solid var(--border);
    display: flex;
    flex-direction: column;
    gap: 14px;
    padding: 16px 12px;
    overflow-y: auto;
    flex-shrink: 0;
  }

  .app-header {
    display: flex;
    align-items: center;
    gap: 8px;
    padding-bottom: 10px;
    border-bottom: 1px solid var(--border);
  }
  .logo { font-size: 22px; color: var(--accent); }
  .app-name { font-size: 17px; font-weight: 700; letter-spacing: -0.02em; }

  /* ── Section ── */
  .section {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .section-title {
    font-size: 10px;
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: .08em;
    color: var(--text-muted);
  }

  /* ── Options ── */
  .option-row {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 12px;
    cursor: pointer;
  }
  .option-row.col { flex-direction: column; align-items: stretch; gap: 4px; }
  .option-row input[type=checkbox] { accent-color: var(--accent); }
  .option-row input[type=range] { accent-color: var(--accent); }

  /* ── Stack button ── */
  .stack-btn {
    width: 100%;
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 6px;
    padding: 10px;
    font-size: 14px;
  }

  /* ── Mini spinner ── */
  .mini-spin {
    width: 14px;
    height: 14px;
    border: 2px solid rgba(255,255,255,.3);
    border-top-color: #fff;
    border-radius: 50%;
    display: inline-block;
    animation: spin 0.7s linear infinite;
    flex-shrink: 0;
  }
  @keyframes spin { to { transform: rotate(360deg); } }

  .error-box {
    background: rgba(247, 91, 91, 0.1);
    border: 1px solid var(--danger);
    color: var(--danger);
    border-radius: var(--radius);
    padding: 8px 10px;
    font-size: 12px;
    word-break: break-word;
  }

  .success-msg {
    font-size: 11px;
    color: var(--success);
    word-break: break-all;
  }

  /* ── Main ── */
  .main {
    flex: 1;
    padding: 16px;
    display: flex;
    flex-direction: column;
    min-width: 0;
  }
</style>
