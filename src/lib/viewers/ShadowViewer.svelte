<script lang="ts">
  import { api, type ShadowInfo } from "../api";
  import { errorMessage } from "../format";
  import ImageViewer from "./ImageViewer.svelte";

  let { mft }: { mft: number } = $props();

  let info = $state.raw<ShadowInfo | null>(null);
  let error = $state<string | null>(null);

  $effect(() => {
    const id = mft;
    info = null;
    let stale = false;
    api
      .shadowInfo(id)
      .then((s) => !stale && (info = s))
      .catch((e) => !stale && (error = errorMessage(e)));
    return () => (stale = true);
  });

  const vec = (v: [number, number, number]) => v.map((x) => Math.round(x * 100) / 100).join(", ");
</script>

<div class="bar">
  {#if error}
    <span class="error">{error}</span>
  {:else if info}
    <span>
      {info.width} × {info.height} · {info.tileWidth}×{info.tileHeight} tiles: {info.uniformTiles.toLocaleString()} uniform,
      {info.detailTiles.toLocaleString()} detailed
    </span>
    <span class="muted" title="Light position and direction in world units">
      sun at {vec(info.eye)} looking {vec(info.direction)}
    </span>
  {/if}
</div>
<div class="note muted">
  White is lit. Detailed tiles are shown at the middle of their stored value range: their per-pixel encoding is not
  decoded yet.
</div>
<ImageViewer load={() => api.shadowImage(mft)} />

<style>
  .bar {
    display: flex;
    align-items: center;
    gap: 16px;
    min-height: 34px;
    padding: 4px 12px;
    background: var(--panel);
    border-bottom: 1px solid var(--border);
    flex: none;
    flex-wrap: wrap;
  }

  .note {
    padding: 4px 12px;
    font-size: 11px;
    background: var(--panel);
    border-bottom: 1px solid var(--border);
    flex: none;
  }

  .error {
    color: var(--accent);
  }
</style>
