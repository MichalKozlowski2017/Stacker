<script lang="ts">
  import type { ImageInfo } from "../types";

  let {
    images,
    onRemove,
    onClear,
  }: {
    images: ImageInfo[];
    onRemove: (index: number) => void;
    onClear: () => void;
  } = $props();
</script>

<div class="image-list">
  <div class="list-header">
    <span class="count">{images.length} {images.length === 1 ? "zdjęcie" : images.length < 5 ? "zdjęcia" : "zdjęć"}</span>
    {#if images.length > 0}
      <button class="btn-danger" onclick={onClear}>Wyczyść</button>
    {/if}
  </div>

  <div class="items">
    {#each images as img, i (img.path)}
      <div class="item">
        <img
          src="data:image/jpeg;base64,{img.thumbnail}"
          alt={img.path.split("/").at(-1)}
          class="thumb"
        />
        <div class="meta">
          <span class="name">{img.path.split("/").at(-1)}</span>
          <span class="dims">{img.width} × {img.height}</span>
        </div>
        <button class="remove" onclick={() => onRemove(i)} title="Usuń">✕</button>
      </div>
    {/each}
  </div>
</div>

<style>
  .image-list { display: flex; flex-direction: column; gap: 6px; }

  .list-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 0 2px;
  }
  .count { font-weight: 600; color: var(--text-muted); font-size: 11px; text-transform: uppercase; letter-spacing: .05em; }

  .items {
    display: flex;
    flex-direction: column;
    gap: 4px;
    max-height: 340px;
    overflow-y: auto;
  }

  .item {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 6px 8px;
    background: var(--surface2);
    border-radius: 6px;
    border: 1px solid var(--border);
  }

  .thumb {
    width: 44px;
    height: 34px;
    object-fit: cover;
    border-radius: 4px;
    flex-shrink: 0;
  }

  .meta {
    flex: 1;
    overflow: hidden;
    display: flex;
    flex-direction: column;
    gap: 1px;
  }

  .name {
    font-size: 12px;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .dims { font-size: 10px; color: var(--text-muted); }

  .remove {
    background: transparent;
    color: var(--text-muted);
    font-size: 11px;
    padding: 2px 5px;
    border-radius: 4px;
    border: none;
    flex-shrink: 0;
    line-height: 1;
  }
  .remove:hover { color: var(--danger); background: rgba(247,91,91,.1); }
</style>
