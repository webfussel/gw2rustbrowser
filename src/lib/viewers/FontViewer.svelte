<script lang="ts">
  import { api, type FontInfo } from "../api";
  import { errorMessage } from "../format";
  import ImageViewer from "./ImageViewer.svelte";

  let { mft }: { mft: number } = $props();

  let fonts = $state.raw<FontInfo[] | null>(null);
  let error = $state<string | null>(null);
  let fontIndex = $state(0);
  let range = $state(0);
  let page = $state(0);
  let pages = $state(1);

  $effect(() => {
    let stale = false;
    api
      .loadFonts(mft)
      .then((f) => !stale && (fonts = f))
      .catch((e) => !stale && (error = errorMessage(e)));
    return () => (stale = true);
  });

  const font = $derived(fonts?.[fontIndex] ?? null);

  function chooseFont(index: number) {
    fontIndex = index;
    const ranges = fonts?.[index]?.ranges ?? [];
    if (!ranges.some((r) => r.range === range)) range = ranges[0]?.range ?? 0;
    page = 0;
  }

  // A fresh loader per selection makes the image viewer reload.
  const load = $derived.by(() => {
    const [f, r, p] = [fontIndex, range, page];
    return async () => {
      const result = await api.fontAtlas(mft, f, r, p);
      pages = result.pages;
      return result.image;
    };
  });
</script>

{#if error}
  <div class="message error">{error}</div>
{:else if !fonts}
  <div class="message muted">Loading…</div>
{:else}
  <div class="layout">
    <div class="fonts">
      {#each fonts as f (f.index)}
        <button class:active={f.index === fontIndex} onclick={() => chooseFont(f.index)}>
          <span>Font {f.index}</span>
          <span class="muted mono">{f.extentX}×{f.extentY}</span>
        </button>
      {/each}
    </div>
    <div class="main">
      <div class="bar">
        <select bind:value={range} onchange={() => (page = 0)}>
          {#each font?.ranges ?? [] as r (r.range)}
            <option value={r.range}>{r.name}</option>
          {/each}
        </select>
        {#if pages > 1}
          <button class="btn small ghost" disabled={page === 0} onclick={() => page--}>‹</button>
          <span class="muted">Page {page + 1} / {pages}</span>
          <button class="btn small ghost" disabled={page >= pages - 1} onclick={() => page++}>›</button>
        {/if}
      </div>
      {#key load}
        <ImageViewer {load} />
      {/key}
    </div>
  </div>
{/if}

<style>
  .layout {
    flex: 1;
    display: flex;
    min-height: 0;
  }

  .fonts {
    width: 150px;
    flex: none;
    overflow: auto;
    background: var(--panel);
    border-right: 1px solid var(--border);
  }

  .fonts button {
    display: flex;
    justify-content: space-between;
    width: 100%;
    border: 0;
    background: none;
    padding: 5px 12px;
    text-align: left;
    cursor: pointer;
  }

  .fonts button:hover {
    background: var(--row-hover);
  }

  .fonts button.active {
    background: var(--accent-soft);
    box-shadow: inset 2px 0 var(--accent);
  }

  .main {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
  }

  .bar {
    display: flex;
    align-items: center;
    gap: 8px;
    height: 40px;
    padding: 0 12px;
    background: var(--panel);
    border-bottom: 1px solid var(--border);
    flex: none;
  }

  select {
    height: 28px;
    padding: 0 8px;
    border: 1px solid var(--border);
    border-radius: var(--radius);
    background: var(--panel-2);
    color: var(--text);
    font: inherit;
  }

  .message {
    flex: 1;
    display: grid;
    place-items: center;
  }

  .error {
    color: var(--accent);
  }
</style>
