<script lang="ts">
  import { api, type DecodedImage, type PakInfo, type PakResource } from "../api";
  import { errorMessage, formatBytes, formatNumber } from "../format";
  import ImageViewer from "./ImageViewer.svelte";
  import TextViewer from "./TextViewer.svelte";
  import HexViewer from "./HexViewer.svelte";

  let { mft }: { mft: number } = $props();

  let info = $state.raw<PakInfo | null>(null);
  let error = $state<string | null>(null);
  let kind = $state<string | null>(null);
  let filter = $state("");
  let selected = $state<PakResource | null>(null);
  let raw = $state(false);

  $effect(() => {
    const id = mft;
    info = null;
    error = null;
    selected = null;
    let stale = false;
    api
      .pakInfo(id)
      .then((p) => {
        if (stale) return;
        info = p;
        selected = p.resources[0] ?? null;
      })
      .catch((e) => !stale && (error = errorMessage(e)));
    return () => (stale = true);
  });

  const kinds = $derived.by(() => {
    const counts = new Map<string, number>();
    for (const r of info?.resources ?? []) counts.set(r.kind, (counts.get(r.kind) ?? 0) + 1);
    return [...counts.entries()].sort((a, b) => b[1] - a[1]);
  });
  const shown = $derived(
    (info?.resources ?? []).filter((r) => (kind === null || r.kind === kind) && String(r.id).startsWith(filter.trim())),
  );

  const imageTypes: Record<string, string> = {
    png: "image/png",
    jpeg: "image/jpeg",
    gif: "image/gif",
    webp: "image/webp",
    svg: "image/svg+xml",
  };
  const languages: Record<string, string | null> = {
    html: "xml",
    svg: "xml",
    json: "json",
    css: "css",
    javascript: "javascript",
    text: null,
  };

  /** Decodes an image with the web view's own decoders. */
  async function decodeImage(bytes: Uint8Array, type: string): Promise<DecodedImage> {
    const url = URL.createObjectURL(new Blob([bytes.slice().buffer], { type }));
    try {
      const img = new Image();
      img.src = url;
      await img.decode();
      // SVG icons are tiny (often 24 px): raster them larger so they stay sharp.
      const scale = type === "image/svg+xml" ? Math.max(1, 512 / Math.max(img.naturalWidth, img.naturalHeight, 1)) : 1;
      const width = Math.round((img.naturalWidth || 300) * scale);
      const height = Math.round((img.naturalHeight || 150) * scale);
      const canvas = new OffscreenCanvas(width, height);
      const ctx = canvas.getContext("2d")!;
      ctx.drawImage(img, 0, 0, width, height);
      const pixels = ctx.getImageData(0, 0, width, height).data;
      return { width, height, format: type.split("/")[1].replace("+xml", "").toUpperCase(), pixels };
    } finally {
      URL.revokeObjectURL(url);
    }
  }

  const load = (r: PakResource) => api.pakResource(mft, r.id);
</script>

{#if error}
  <div class="message error">{error}</div>
{:else if !info}
  <div class="message muted">Loading…</div>
{:else}
  <div class="bar">
    <span class="muted">
      Chromium resource pack v{info.version} · {formatNumber(info.resources.length)} resources
    </span>
    <input type="search" placeholder="Resource id" bind:value={filter} />
    <span class="kinds">
      <button class="btn small" class:ghost={kind !== null} onclick={() => (kind = null)}>All</button>
      {#each kinds as [k, n] (k)}
        <button class="btn small" class:ghost={kind !== k} onclick={() => (kind = k)}>{k} ({n})</button>
      {/each}
    </span>
  </div>
  <div class="split">
    <div class="list">
      {#each shown.slice(0, 5000) as r (r.id)}
        <button class="row" class:active={selected?.id === r.id} onclick={() => (selected = r)}>
          <span class="mono id">{r.id}</span>
          <span class="kind">{r.kind}</span>
          <span class="muted size">{formatBytes(r.size)}</span>
        </button>
      {/each}
    </div>
    <div class="preview">
      {#if selected}
        {@const r = selected}
        <div class="meta">
          <span>Resource <span class="mono">{r.id}</span></span>
          <span class="muted">
            {r.kind} · {formatBytes(r.size)}{r.compression ? ` stored, ${r.compression}-compressed` : ""}{r.aliasOf !== null
              ? ` · alias of ${r.aliasOf}`
              : ""}
          </span>
          {#if r.kind in languages || r.kind in imageTypes}
            <label><input type="checkbox" bind:checked={raw} /> Hex</label>
          {/if}
        </div>
        {#key `${r.id}-${raw}`}
          {#if !raw && r.kind in imageTypes}
            <ImageViewer load={async () => decodeImage(await load(r), imageTypes[r.kind])} />
          {:else if !raw && r.kind in languages}
            <TextViewer
              language={languages[r.kind]}
              load={async () => {
                const bytes = await load(r);
                return new TextDecoder(info?.encoding === 2 ? "utf-16le" : "utf-8").decode(bytes);
              }}
            />
          {:else}
            <HexViewer load={() => load(r)} />
          {/if}
        {/key}
      {/if}
    </div>
  </div>
{/if}

<style>
  .bar {
    display: flex;
    align-items: center;
    gap: 12px;
    min-height: 44px;
    padding: 4px 12px;
    background: var(--panel);
    border-bottom: 1px solid var(--border);
    flex: none;
    flex-wrap: wrap;
  }

  .bar input {
    width: 120px;
  }

  .kinds {
    display: flex;
    flex-wrap: wrap;
    gap: 4px;
  }

  .split {
    flex: 1;
    display: flex;
    min-height: 0;
  }

  .list {
    width: 240px;
    flex: none;
    overflow: auto;
    border-right: 1px solid var(--border);
    background: var(--panel);
  }

  .row {
    display: grid;
    grid-template-columns: 52px 1fr auto;
    gap: 8px;
    width: 100%;
    padding: 3px 10px;
    border: none;
    background: none;
    color: inherit;
    text-align: left;
    font: inherit;
    cursor: pointer;
  }

  .row:hover {
    background: var(--panel-2);
  }

  .row.active {
    background: color-mix(in srgb, var(--accent) 18%, transparent);
  }

  .size {
    font-size: 11px;
  }

  .preview {
    flex: 1;
    display: flex;
    flex-direction: column;
    min-width: 0;
  }

  .meta {
    display: flex;
    align-items: center;
    gap: 12px;
    height: 34px;
    padding: 0 12px;
    background: var(--panel);
    border-bottom: 1px solid var(--border);
    flex: none;
  }

  .meta label {
    display: flex;
    align-items: center;
    gap: 6px;
    margin-left: auto;
  }

  .message {
    padding: 24px;
    text-align: center;
  }

  .error {
    color: var(--accent);
  }
</style>
