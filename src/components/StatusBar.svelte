<script lang="ts">
  type Phase = "idle" | "loading" | "stacking" | "done" | "error";

  let {
    phase,
    loadCurrent = 0,
    loadTotal = 0,
    loadFile = "",
    stackElapsed = 0,
    stackPhase = "",
    stackCurrent = 0,
    stackTotal = 0,
    resultWidth = 0,
    resultHeight = 0,
    errorMsg = "",
  }: {
    phase: Phase;
    loadCurrent?: number;
    loadTotal?: number;
    loadFile?: string;
    stackElapsed?: number;
    stackPhase?: string;
    stackCurrent?: number;
    stackTotal?: number;
    resultWidth?: number;
    resultHeight?: number;
    errorMsg?: string;
  } = $props();

  function fmt(secs: number) {
    const m = Math.floor(secs / 60).toString().padStart(2, "0");
    const s = (secs % 60).toString().padStart(2, "0");
    return `${m}:${s}`;
  }

  const PHASE_LABELS: Record<string, string> = {
    loading:  "Wczytywanie",
    aligning: "Wyrównywanie",
    sharpness:"Mapy ostrości",
    blending: "Łączenie",
    encoding: "Kodowanie podglądu",
  };

  const loadPct   = $derived(loadTotal   > 0 ? (loadCurrent  / loadTotal)   * 100 : 0);
  const stackPct  = $derived(stackTotal  > 0 ? (stackCurrent / stackTotal)  * 100 : 0);
  const phaseLabel = $derived(PHASE_LABELS[stackPhase] ?? stackPhase);
  const hasStackProgress = $derived(stackTotal > 0);
</script>

{#if phase !== "idle"}
  <div class="statusbar" class:error={phase === "error"} class:done={phase === "done"}>

    <!-- Left: icon + text -->
    <div class="info">
      {#if phase === "loading"}
        <span class="dot pulse blue"></span>
        <span>
          Ładowanie&nbsp;
          <strong>{loadCurrent}/{loadTotal}</strong>
          {#if loadFile}
            &nbsp;·&nbsp;<span class="filename">{loadFile}</span>
          {/if}
        </span>
      {:else if phase === "stacking"}
        <span class="dot pulse accent"></span>
        <span>
          {#if phaseLabel}
            <strong>{phaseLabel}</strong>
            {#if stackTotal > 1}
              &nbsp;{stackCurrent}/{stackTotal}
            {/if}
            &nbsp;·&nbsp;
          {/if}
          <strong>{fmt(stackElapsed)}</strong>
        </span>
      {:else if phase === "done"}
        <span class="dot green"></span>
        <span>
          Gotowe&nbsp;·&nbsp;{resultWidth}&nbsp;×&nbsp;{resultHeight}&nbsp;px
          &nbsp;·&nbsp;<strong>{fmt(stackElapsed)}</strong>
        </span>
      {:else if phase === "error"}
        <span class="dot red"></span>
        <span class="err-text">{errorMsg}</span>
      {/if}
    </div>

    <!-- Right: progress bar -->
    <div class="bar-wrap">
      {#if phase === "loading"}
        <div class="bar-track">
          <div class="bar-fill" style="width: {loadPct}%"></div>
        </div>
      {:else if phase === "stacking"}
        <div class="bar-track">
          {#if hasStackProgress}
            <div class="bar-fill" style="width: {stackPct}%"></div>
          {:else}
            <div class="bar-indeterminate"></div>
          {/if}
        </div>
        {#if hasStackProgress}
          <div class="pct-label">{Math.round(stackPct)}%</div>
        {/if}
      {:else if phase === "done"}
        <div class="bar-track">
          <div class="bar-fill done" style="width: 100%"></div>
        </div>
      {/if}
    </div>
  </div>
{/if}

<style>
  .statusbar {
    height: 32px;
    background: var(--surface);
    border-top: 1px solid var(--border);
    display: flex;
    align-items: center;
    gap: 16px;
    padding: 0 14px;
    font-size: 12px;
    color: var(--text-muted);
    flex-shrink: 0;
  }
  .statusbar.done { color: var(--text); }
  .statusbar.error { background: rgba(247,91,91,.07); }

  .info {
    display: flex;
    align-items: center;
    gap: 6px;
    flex: 1;
    overflow: hidden;
    white-space: nowrap;
  }

  .filename {
    opacity: .75;
    overflow: hidden;
    text-overflow: ellipsis;
    max-width: 220px;
    display: inline-block;
    vertical-align: bottom;
  }

  .err-text {
    color: var(--danger);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  /* Dots */
  .dot {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    flex-shrink: 0;
    display: inline-block;
  }
  .dot.blue    { background: #4f8ef7; }
  .dot.accent  { background: var(--accent); }
  .dot.green   { background: var(--success); }
  .dot.red     { background: var(--danger); }

  .dot.pulse {
    animation: pulse 1.2s ease-in-out infinite;
  }
  @keyframes pulse {
    0%, 100% { opacity: 1; transform: scale(1); }
    50%       { opacity: .5; transform: scale(.75); }
  }

  /* Progress bar */
  .bar-wrap {
    display: flex;
    align-items: center;
    gap: 6px;
    flex-shrink: 0;
  }

  .bar-track {
    width: 160px;
    height: 4px;
    background: var(--surface2);
    border-radius: 2px;
    overflow: hidden;
    position: relative;
  }

  .bar-fill {
    height: 100%;
    background: var(--accent);
    border-radius: 2px;
    transition: width 0.25s ease;
  }
  .bar-fill.done { background: var(--success); }

  .pct-label {
    font-size: 11px;
    color: var(--text-muted);
    width: 28px;
    text-align: right;
    flex-shrink: 0;
  }

  /* Indeterminate shimmer */
  .bar-indeterminate {
    position: absolute;
    top: 0;
    left: -60%;
    width: 60%;
    height: 100%;
    background: linear-gradient(
      90deg,
      transparent 0%,
      var(--accent) 40%,
      #7fb7ff 60%,
      transparent 100%
    );
    border-radius: 2px;
    animation: slide 1.4s linear infinite;
  }
  @keyframes slide {
    from { left: -60%; }
    to   { left: 110%; }
  }
</style>
