<script lang="ts">
  import { api, type GlyphFileInfo } from "../api";
  import { errorMessage } from "../format";
  import ImageViewer from "./ImageViewer.svelte";

  let { mft }: { mft: number } = $props();

  let info = $state<GlyphFileInfo | null>(null);
  let error = $state<string | null>(null);
  let page = $state(0);
  let pages = $state(1);

  $effect(() => {
    let stale = false;
    page = 0;
    api
      .glyphFileInfo(mft)
      .then((i) => !stale && (info = i))
      .catch((e) => !stale && (error = errorMessage(e)));
    return () => (stale = true);
  });

  const load = $derived.by(() => {
    const p = page;
    return async () => {
      const result = await api.glyphFileAtlas(mft, p);
      pages = result.pages;
      return result.image;
    };
  });

  const hex = (c: number) => `U+${c.toString(16).toUpperCase().padStart(4, "0")}`;
</script>

{#if error}
  <div class="message error">{error}</div>
{:else}
  <div class="bar">
    {#if info}
      <span>{info.rangeName}</span>
      <span class="muted mono">{hex(info.first)}–{hex(info.last)}</span>
      <span class="muted">used by font{info.fonts.length === 1 ? "" : "s"} {info.fonts.join(", ")}</span>
    {/if}
    {#if pages > 1}
      <span class="pager">
        <button class="btn small ghost" disabled={page === 0} onclick={() => page--}>‹</button>
        <span class="muted">Page {page + 1} / {pages}</span>
        <button class="btn small ghost" disabled={page >= pages - 1} onclick={() => page++}>›</button>
      </span>
    {/if}
  </div>
  {#key load}
    <ImageViewer {load} />
  {/key}
{/if}

<style>
  .bar {
    display: flex;
    align-items: center;
    gap: 12px;
    height: 40px;
    padding: 0 12px;
    background: var(--panel);
    border-bottom: 1px solid var(--border);
    flex: none;
  }

  .pager {
    margin-left: auto;
    display: flex;
    align-items: center;
    gap: 6px;
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
