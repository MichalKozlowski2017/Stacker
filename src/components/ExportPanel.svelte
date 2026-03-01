<script lang="ts">
  import { save } from "@tauri-apps/plugin-dialog";
  import { invoke } from "@tauri-apps/api/core";

  let {
    hasResult,
    onExported,
  }: {
    hasResult: boolean;
    onExported: (path: string) => void;
  } = $props();

  let format = $state<"png" | "jpg" | "tiff">("png");
  let quality = $state(92);
  let exporting = $state(false);
  let error = $state<string | null>(null);

  async function exportImage() {
    exporting = true;
    error = null;

    try {
      const ext = format;
      const path = await save({
        defaultPath: `stacked.${ext}`,
        filters: [{ name: ext.toUpperCase(), extensions: [ext] }],
      });
      if (!path) return;

      await invoke("export_image", { outputPath: path, quality });
      onExported(path);
    } catch (e: any) {
      error = e?.message ?? String(e);
    } finally {
      exporting = false;
    }
  }
</script>

<div class="export-panel">
  <div class="row">
    <span class="lbl">Format</span>
    <div class="seg">
      {#each (["png", "jpg", "tiff"] as const) as f}
        <button
          class:active={format === f}
          onclick={() => (format = f)}
        >{f.toUpperCase()}</button>
      {/each}
    </div>
  </div>

  {#if format === "jpg"}
    <div class="row">
      <label for="quality-range">Jakość&nbsp;<span class="val">{quality}%</span></label>
      <input id="quality-range" type="range" min="50" max="100" bind:value={quality} />
    </div>
  {/if}

  <button
    class="btn-primary export-btn"
    disabled={!hasResult || exporting}
    onclick={exportImage}
  >
    {exporting ? "Eksportowanie…" : "Eksportuj"}
  </button>

  {#if error}
    <p class="error">{error}</p>
  {/if}
</div>

<style>
  .export-panel { display: flex; flex-direction: column; gap: 10px; }

  .row {
    display: flex;
    align-items: center;
    gap: 8px;
    flex-wrap: wrap;
  }

  .lbl {
    font-size: 12px;
    color: var(--text-muted);
    width: 64px;
    flex-shrink: 0;
  }

  .seg { display: flex; gap: 4px; }
  .seg button {
    background: var(--surface2);
    border: 1px solid var(--border);
    color: var(--text-muted);
    padding: 4px 10px;
    font-size: 11px;
    font-weight: 600;
    border-radius: 5px;
  }
  .seg button.active {
    background: var(--accent-dim);
    border-color: var(--accent);
    color: var(--accent);
  }
  .seg button:hover:not(.active) { background: #2f2f2f; }

  input[type=range] {
    flex: 1;
    accent-color: var(--accent);
  }

  .val { color: var(--text); font-weight: 600; }

  .export-btn { width: 100%; text-align: center; }

  .error { color: var(--danger); font-size: 12px; }
</style>
