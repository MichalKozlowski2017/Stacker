<script lang="ts">
  let {
    preview,
    width,
    height,
    loading,
  }: {
    preview: string | null;
    width: number;
    height: number;
    loading: boolean;
  } = $props();
</script>

<div class="preview-wrap">
  {#if loading}
    <div class="placeholder">
      <div class="spinner"></div>
      <p>Stackowanie w toku…</p>
    </div>
  {:else if preview}
    <img
      src="data:image/png;base64,{preview}"
      alt="Focus-stacked result"
      class="result-img"
    />
    <div class="badge">{width} × {height} px</div>
  {:else}
    <div class="placeholder empty">
      <svg width="48" height="48" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1">
        <rect x="3" y="3" width="18" height="18" rx="2" />
        <circle cx="8.5" cy="8.5" r="1.5" />
        <path stroke-linecap="round" d="M21 15l-5-5L5 21" />
      </svg>
      <p>Tutaj pojawi się wynik</p>
    </div>
  {/if}
</div>

<style>
  .preview-wrap {
    position: relative;
    flex: 1;
    display: flex;
    align-items: center;
    justify-content: center;
    background: var(--surface);
    border-radius: var(--radius);
    border: 1px solid var(--border);
    overflow: hidden;
    min-height: 0;
  }

  .result-img {
    max-width: 100%;
    max-height: 100%;
    object-fit: contain;
    display: block;
    border-radius: 4px;
  }

  .badge {
    position: absolute;
    bottom: 10px;
    right: 10px;
    background: rgba(0,0,0,.65);
    color: var(--text-muted);
    font-size: 11px;
    padding: 3px 8px;
    border-radius: 20px;
    backdrop-filter: blur(4px);
  }

  .placeholder {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 14px;
    color: var(--text-muted);
  }

  .empty svg { opacity: .3; }

  /* Spinner */
  .spinner {
    width: 36px;
    height: 36px;
    border: 3px solid var(--border);
    border-top-color: var(--accent);
    border-radius: 50%;
    animation: spin 0.8s linear infinite;
  }
  @keyframes spin { to { transform: rotate(360deg); } }
</style>
