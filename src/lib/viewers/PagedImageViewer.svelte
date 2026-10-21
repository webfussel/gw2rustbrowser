<script lang="ts">
  import { onMount } from "svelte";
  import { save } from "@tauri-apps/plugin-dialog";
  import { api, type PagedImageLayer } from "../api";
  import { errorMessage } from "../format";
  import { notify } from "../state.svelte";

  let { mft, name }: { mft: number; name: string } = $props();

  const OVERVIEW_SIZE = 2048;
  const MAX_TILES = 300;
  const MAX_LOADS = 6;
  const EXPORT_LIMIT = 16384;

  let layers = $state.raw<PagedImageLayer[] | null>(null);
  let layerIndex = $state(0);
  let error = $state<string | null>(null);
  let loadingOverview = $state(false);
  let hover = $state<string | null>(null);
  let exportPixels = $state(0);
  let exporting = $state(false);
  let zoomText = $state("");

  let container: HTMLDivElement | undefined = $state();
  let canvas: HTMLCanvasElement | undefined = $state();
  let overview: ImageBitmap | null = null;
  /** Overview pixels per page. */
  let overviewPixels = 1;
  /** Screen (CSS) pixels per page and screen position of the layer's top-left page. */
  let scale = 1;
  let ox = 0;
  let oy = 0;
  let fitScale = 1;
  const tiles = new Map<string, ImageBitmap>();
  const loading = new Set<string>();
  let frame = 0;
  let disposed = false;

  const layer = $derived(layers?.[layerIndex] ?? null);
  const grid = $derived.by(() => {
    const b = layer?.bounds;
    return b ? { x0: b[0], y0: b[1], columns: b[2] - b[0] + 1, rows: b[3] - b[1] + 1 } : null;
  });
  /** Page index by grid cell, for hover and tile lookup. */
  const cells = $derived.by(() => {
    const map = new Map<number, number>();
    if (layer) layer.xs.forEach((x, i) => map.set(x * 65536 + layer.ys[i], i));
    return map;
  });
  const exportOptions = $derived.by(() => {
    if (!layer || !grid) return [];
    const longest = Math.max(grid.columns, grid.rows);
    return [32, 64, 128, 256, 512, 1024]
      .filter((px) => px <= layer.pageWidth)
      .map((px) => ({ px, width: grid.columns * px, height: grid.rows * px, allowed: longest * px <= EXPORT_LIMIT }));
  });

  $effect(() => {
    const id = mft;
    layers = null;
    error = null;
    layerIndex = 0;
    let stale = false;
    api
      .pagedImageInfo(id)
      .then((list) => !stale && (layers = list))
      .catch((e) => !stale && (error = errorMessage(e)));
    return () => (stale = true);
  });

  // New layer: load its overview and fit it into the view.
  $effect(() => {
    const current = layer;
    const g = grid;
    if (!current || !g) return;
    let stale = false;
    clearTiles();
    overview?.close();
    overview = null;
    loadingOverview = true;
    api
      .pagedImageOverview(mft, current.index, OVERVIEW_SIZE)
      .then(async (image) => {
        const bitmap = await createImageBitmap(new ImageData(image.pixels, image.width, image.height));
        if (stale || disposed) return bitmap.close();
        overview = bitmap;
        overviewPixels = image.width / g.columns;
        const best = exportOptions.filter((o) => o.allowed).at(-1);
        exportPixels = best ? Math.min(best.px, 256) : 0;
        fit();
      })
      .catch((e) => !stale && (error = errorMessage(e)))
      .finally(() => !stale && (loadingOverview = false));
    return () => (stale = true);
  });

  function clearTiles() {
    for (const t of tiles.values()) t.close();
    tiles.clear();
    loading.clear();
  }

  function fit() {
    if (!canvas || !grid) return;
    const { clientWidth: w, clientHeight: h } = canvas;
    fitScale = Math.min(w / grid.columns, h / grid.rows) * 0.96;
    scale = fitScale;
    ox = (w - grid.columns * scale) / 2;
    oy = (h - grid.rows * scale) / 2;
    requestDraw();
  }

  function requestDraw() {
    if (frame) return;
    frame = requestAnimationFrame(() => {
      frame = 0;
      draw();
    });
  }

  function tileSize(pixels: number, page: number) {
    let size = 64;
    while (size < pixels && size < page) size *= 2;
    return Math.min(size, page);
  }

  function draw() {
    if (!canvas || !layer || !grid) return;
    const dpr = window.devicePixelRatio || 1;
    const w = canvas.clientWidth;
    const h = canvas.clientHeight;
    if (canvas.width !== Math.round(w * dpr) || canvas.height !== Math.round(h * dpr)) {
      canvas.width = Math.round(w * dpr);
      canvas.height = Math.round(h * dpr);
    }
    const ctx = canvas.getContext("2d")!;
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    ctx.clearRect(0, 0, w, h);
    ctx.imageSmoothingEnabled = true;
    ctx.imageSmoothingQuality = "high";
    if (overview) ctx.drawImage(overview, ox, oy, grid.columns * scale, grid.rows * scale);
    zoomText = `${Math.round((scale / layer.pageWidth) * 100)} %`;

    // Sharper pages once the overview is magnified.
    if (scale * dpr <= overviewPixels * 1.25) return;
    const size = tileSize(scale * dpr, layer.pageWidth);
    const firstX = Math.max(0, Math.floor(-ox / scale));
    const firstY = Math.max(0, Math.floor(-oy / scale));
    const lastX = Math.min(grid.columns - 1, Math.floor((w - ox) / scale));
    const lastY = Math.min(grid.rows - 1, Math.floor((h - oy) / scale));
    const wanted: string[] = [];
    for (let y = firstY; y <= lastY; y++) {
      for (let x = firstX; x <= lastX; x++) {
        const i = cells.get((x + grid.x0) * 65536 + y + grid.y0);
        if (i === undefined) continue;
        const sx = ox + x * scale;
        const sy = oy + y * scale;
        const file = layer.files[i];
        if (!file) {
          const c = layer.colors[i];
          ctx.fillStyle = `rgba(${c >>> 24}, ${(c >>> 16) & 255}, ${(c >>> 8) & 255}, ${(c & 255) / 255})`;
          ctx.fillRect(sx, sy, scale + 0.5, scale + 0.5);
          continue;
        }
        const key = `${file}:${size}`;
        const tile = tiles.get(key) ?? fallback(file, size);
        if (tile) ctx.drawImage(tile, sx, sy, scale + 0.5, scale + 0.5);
        if (!tiles.has(key)) wanted.push(key);
        else {
          // Most recently used last.
          const t = tiles.get(key)!;
          tiles.delete(key);
          tiles.set(key, t);
        }
      }
    }
    for (const key of wanted) {
      if (loading.size >= MAX_LOADS) break;
      if (!loading.has(key)) load(key);
    }
  }

  /** Any already loaded resolution of a page, while the wanted one loads. */
  function fallback(file: number, size: number) {
    for (let s = size / 2; s >= 64; s /= 2) {
      const t = tiles.get(`${file}:${s}`);
      if (t) return t;
    }
    return null;
  }

  async function load(key: string) {
    loading.add(key);
    const [file, size] = key.split(":").map(Number);
    try {
      const image = await api.pagedImageTile(file, size);
      const bitmap = await createImageBitmap(new ImageData(image.pixels, image.width, image.height));
      if (disposed || !loading.has(key)) return bitmap.close();
      tiles.set(key, bitmap);
      while (tiles.size > MAX_TILES) {
        const [oldest, t] = tiles.entries().next().value!;
        t.close();
        tiles.delete(oldest);
      }
    } catch {
      // A broken page keeps showing the overview.
    } finally {
      loading.delete(key);
      requestDraw();
    }
  }

  function zoomAt(factor: number, cx: number, cy: number) {
    if (!layer) return;
    const next = Math.min(Math.max(scale * factor, fitScale * 0.5), layer.pageWidth * 4);
    ox = cx - ((cx - ox) * next) / scale;
    oy = cy - ((cy - oy) * next) / scale;
    scale = next;
    requestDraw();
  }

  function onWheel(event: WheelEvent) {
    event.preventDefault();
    const rect = canvas!.getBoundingClientRect();
    zoomAt(Math.pow(1.0015, -event.deltaY), event.clientX - rect.left, event.clientY - rect.top);
  }

  let drag: { x: number; y: number } | null = null;
  function onPointerDown(event: PointerEvent) {
    drag = { x: event.clientX, y: event.clientY };
    canvas!.setPointerCapture(event.pointerId);
  }
  function onPointerMove(event: PointerEvent) {
    if (drag) {
      ox += event.clientX - drag.x;
      oy += event.clientY - drag.y;
      drag = { x: event.clientX, y: event.clientY };
      requestDraw();
    }
    if (!layer || !grid) return;
    const rect = canvas!.getBoundingClientRect();
    const gx = Math.floor((event.clientX - rect.left - ox) / scale) + grid.x0;
    const gy = Math.floor((event.clientY - rect.top - oy) / scale) + grid.y0;
    const i = cells.get(gx * 65536 + gy);
    if (i === undefined) {
      hover = null;
      return;
    }
    const file = layer.files[i];
    const c = layer.colors[i];
    hover = file
      ? `page ${gx}, ${gy} · texture ${file}`
      : `page ${gx}, ${gy} · solid #${(c >>> 8).toString(16).padStart(6, "0")}`;
  }
  function onPointerUp() {
    drag = null;
  }

  async function exportImage() {
    if (!layer || !exportPixels) return;
    const path = await save({ defaultPath: `${name}_layer${layer.index}.png`, filters: [{ name: "PNG", extensions: ["png"] }] });
    if (!path) return;
    exporting = true;
    try {
      await api.exportPagedImage(mft, layer.index, exportPixels, path);
      notify(`Exported to ${path}`);
    } catch (e) {
      notify(errorMessage(e), "error");
    } finally {
      exporting = false;
    }
  }

  onMount(() => {
    const resize = new ResizeObserver(() => requestDraw());
    if (container) resize.observe(container);
    return () => {
      disposed = true;
      resize.disconnect();
      cancelAnimationFrame(frame);
      clearTiles();
      overview?.close();
    };
  });
</script>

{#if error}
  <div class="message error">{error}</div>
{:else if !layers}
  <div class="message muted">Loading…</div>
{:else}
  <div class="bar">
    {#if layers.length > 1}
      <span class="group">
        {#each layers as l, i (l.index)}
          <button class="btn small" class:ghost={layerIndex !== i} onclick={() => (layerIndex = i)}>
            Layer {l.index} · {l.format}
          </button>
        {/each}
      </span>
    {/if}
    {#if layer && grid}
      <span class="muted">
        {layer.xs.length.toLocaleString()} pages · {grid.columns} × {grid.rows} grid · page {layer.pageWidth} × {layer.pageHeight}
        {layer.format} · full size {(grid.columns * layer.pageWidth).toLocaleString()} × {(grid.rows * layer.pageHeight).toLocaleString()}
        px
      </span>
    {/if}
    <span class="spacer"></span>
    <button class="btn small ghost" onclick={() => zoomAt(1 / 1.5, canvas!.clientWidth / 2, canvas!.clientHeight / 2)}>−</button>
    <span class="mono zoom">{zoomText}</span>
    <button class="btn small ghost" onclick={() => zoomAt(1.5, canvas!.clientWidth / 2, canvas!.clientHeight / 2)}>+</button>
    <button class="btn small ghost" onclick={fit}>Fit</button>
    <select bind:value={exportPixels} title="Export resolution (pixels per page)">
      {#each exportOptions as o (o.px)}
        <option value={o.px} disabled={!o.allowed}>
          {o.px} px/page · {o.width.toLocaleString()} × {o.height.toLocaleString()}
        </option>
      {/each}
    </select>
    <button class="btn small" onclick={exportImage} disabled={exporting || !exportPixels}>
      {exporting ? "Exporting…" : "Export PNG"}
    </button>
  </div>
  <div class="stage" bind:this={container}>
    <canvas
      bind:this={canvas}
      onwheel={onWheel}
      onpointerdown={onPointerDown}
      onpointermove={onPointerMove}
      onpointerup={onPointerUp}
      onpointerleave={() => (hover = null)}
    ></canvas>
    {#if loadingOverview}
      <div class="overlay muted">Assembling pages…</div>
    {/if}
  </div>
  <div class="hint muted">
    {hover ?? "Drag to pan · scroll to zoom · sharper pages load as you zoom in"}
  </div>
{/if}

<style>
  .bar {
    display: flex;
    align-items: center;
    gap: 10px;
    min-height: 40px;
    padding: 4px 12px;
    background: var(--panel);
    border-bottom: 1px solid var(--border);
    flex: none;
    flex-wrap: wrap;
  }

  .group {
    display: flex;
    gap: 4px;
  }

  .spacer {
    flex: 1;
  }

  .zoom {
    min-width: 52px;
    text-align: center;
  }

  select {
    height: 26px;
    padding: 0 6px;
    border: 1px solid var(--border);
    border-radius: var(--radius);
    background: var(--panel-2);
    color: var(--text);
    font: inherit;
  }

  .stage {
    flex: 1;
    position: relative;
    min-height: 0;
    overflow: hidden;
    background: repeating-conic-gradient(#1b1e23 0% 25%, #22262c 0% 50%) 0 0 / 24px 24px;
  }

  canvas {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    cursor: grab;
    touch-action: none;
  }

  canvas:active {
    cursor: grabbing;
  }

  .overlay {
    position: absolute;
    inset: 0;
    display: grid;
    place-items: center;
    pointer-events: none;
  }

  .hint {
    padding: 4px 12px;
    font-size: 11px;
    background: var(--panel);
    border-top: 1px solid var(--border);
  }

  .message {
    padding: 24px;
    text-align: center;
  }

  .error {
    color: var(--accent);
  }
</style>
