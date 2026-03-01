<script lang="ts">
  import { open } from "@tauri-apps/plugin-dialog";

  let { onFiles }: { onFiles: (paths: string[]) => void } = $props();

  let dragging = $state(false);

  async function openDialog() {
    const selected = await open({
      multiple: true,
      filters: [
        {
          name: "Images",
          extensions: [
            "jpg", "jpeg", "png", "tif", "tiff",
            "cr2", "cr3", "nef", "nrw", "arw", "dng",
            "orf", "rw2", "raf", "pef", "dcr",
          ],
        },
      ],
    });
    if (!selected) return;
    const paths = Array.isArray(selected) ? selected : [selected];
    onFiles(paths);
  }

  function handleDrop(e: DragEvent) {
    e.preventDefault();
    dragging = false;
    const files = Array.from(e.dataTransfer?.files ?? []);
    const paths = files.map((f) => f.path ?? (f as any).name).filter(Boolean);
    if (paths.length) onFiles(paths);
  }
</script>

<!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions -->
<div
  class="drop-zone"
  class:dragging
  onclick={openDialog}
  ondragover={(e) => { e.preventDefault(); dragging = true; }}
  ondragleave={() => (dragging = false)}
  ondrop={handleDrop}
>
  <svg width="40" height="40" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5">
    <path stroke-linecap="round" stroke-linejoin="round"
      d="M3 16.5v2.25A2.25 2.25 0 005.25 21h13.5A2.25 2.25 0 0021 18.75V16.5m-13.5-9L12 3m0 0l4.5 4.5M12 3v13.5" />
  </svg>
  <p>Przeciągnij zdjęcia lub kliknij, żeby wybrać</p>
  <p class="hint">JPG · PNG · TIFF · CR2 · NEF · ARW · DNG i inne RAW</p>
</div>

<style>
  .drop-zone {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 8px;
    padding: 28px 16px;
    border: 2px dashed var(--border);
    border-radius: var(--radius);
    color: var(--text-muted);
    cursor: pointer;
    transition: border-color 0.15s, background 0.15s, color 0.15s;
    text-align: center;
    user-select: none;
  }
  .drop-zone:hover,
  .drop-zone.dragging {
    border-color: var(--accent);
    background: rgba(79, 142, 247, 0.06);
    color: var(--text);
  }
  .hint { font-size: 11px; opacity: 0.6; }
</style>
