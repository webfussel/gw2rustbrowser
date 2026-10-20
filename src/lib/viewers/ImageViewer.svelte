<script lang="ts">
  import type { DecodedImage } from "../api";
  import { errorMessage } from "../format";

  let { load }: { load: () => Promise<DecodedImage> } = $props();

  type Background = "checker" | "dark" | "light";
  const channelNames = ["R", "G", "B", "A"] as const;

  let image = $state.raw<DecodedImage | null>(null);
  let error = $state<string | null>(null);
  let loading = $state(true);
  let channels = $state([true, true, true, true]);
  let background = $state<Background>("checker");
  let zoom = $state(1);
  let offset = $state({ x: 0, y: 0 });
  let fitted = $state(true);
  let box = $state({ width: 0, height: 0 });
  let canvas: HTMLCanvasElement | undefined = $state();
  let bitmap = $state.raw<ImageBitmap | null>(null);

  $effect(() => {
    let stale = false;
    loading = true;
    error = null;
    load()
      .then((img) => {
        if (stale) return;
        image = img;
        fitted = true;
      })
      .catch((e) => !stale && (error = errorMessage(e)))
      .finally(() => !stale && (loading = false));
    return () => (stale = true);
  });

  // Rebuild the displayed bitmap when the image or channel mask changes.
  $effect(() => {
    if (!image) return;
    const [r, g, b, a] = channels;
    const { width, height, pixels } = image;
    let data = pixels;
    if (!(r && g && b && a)) {
      data = new Uint8ClampedArray(pixels);
      const onlyAlpha = a && !r && !g && !b;
      for (let i = 0; i < data.length; i += 4) {
        if (onlyAlpha) {
          data[i] = data[i + 1] = data[i + 2] = pixels[i + 3];
          data[i + 3] = 255;
          continue;
        }
        if (!r) data[i] = 0;
        if (!g) data[i + 1] = 0;
        if (!b) data[i + 2] = 0;
        if (!a) data[i + 3] = 255;
      }
    }
    let stale = false;
    createImageBitmap(new ImageData(data, width, height)).then((bmp) => {
      if (stale) bmp.close();
      else bitmap = bmp;
    });
    return () => (stale = true);
  });

  function fit() {
    if (!image || !box.width) return;
    const scale = Math.min((box.width - 32) / image.width, (box.height - 32) / image.height);
    zoom = Math.min(1, scale) > 0 && scale < 1 ? scale : Math.min(Math.max(1, Math.floor(scale)), 8);
    offset = { x: (box.width - image.width * zoom) / 2, y: (box.height - image.height * zoom) / 2 };
  }

  $effect(() => {
    void box.width;
    void box.height;
    if (fitted && image) fit();
  });

  $effect(() => {
    if (!canvas || !bitmap) return;
    const dpr = window.devicePixelRatio || 1;
    canvas.width = Math.round(box.width * dpr);
    canvas.height = Math.round(box.height * dpr);
    const ctx = canvas.getContext("2d")!;
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    ctx.clearRect(0, 0, box.width, box.height);
    ctx.imageSmoothingEnabled = zoom < 1;
    ctx.imageSmoothingQuality = "high";
    ctx.drawImage(bitmap, offset.x, offset.y, bitmap.width * zoom, bitmap.height * zoom);
    ctx.strokeStyle = "rgba(127,127,127,.35)";
    ctx.strokeRect(offset.x - 0.5, offset.y - 0.5, bitmap.width * zoom + 1, bitmap.height * zoom + 1);
  });

  function setZoom(next: number, cx = box.width / 2, cy = box.height / 2) {
    next = Math.min(64, Math.max(1 / 64, next));
    offset = { x: cx - ((cx - offset.x) * next) / zoom, y: cy - ((cy - offset.y) * next) / zoom };
    zoom = next;
    fitted = false;
  }

  function wheel(event: WheelEvent) {
    event.preventDefault();
    const rect = canvas!.getBoundingClientRect();
    setZoom(zoom * (event.deltaY < 0 ? 1.25 : 0.8), event.clientX - rect.left, event.clientY - rect.top);
  }

  function drag(event: PointerEvent) {
    const target = event.currentTarget as HTMLElement;
    target.setPointerCapture(event.pointerId);
    const start = { x: event.clientX - offset.x, y: event.clientY - offset.y };
    const move = (e: PointerEvent) => {
      offset = { x: e.clientX - start.x, y: e.clientY - start.y };
      fitted = false;
    };
    const stop = () => {
      target.removeEventListener("pointermove", move);
      target.removeEventListener("pointerup", stop);
    };
    target.addEventListener("pointermove", move);
    target.addEventListener("pointerup", stop);
  }

  function toggleChannel(index: number, event: MouseEvent) {
    if (event.shiftKey || event.ctrlKey) {
      // Solo this channel.
      channels = channels.map((_, i) => i === index);
    } else {
      channels[index] = !channels[index];
    }
  }
</script>

<div class="bar">
  <div class="group">
    {#each channelNames as name, i}
      <button
        class="chan chan-{name}"
        class:off={!channels[i]}
        onclick={(e) => toggleChannel(i, e)}
        title="Toggle {name} channel (Shift+click: show only this channel)">{name}</button
      >
    {/each}
  </div>
  <div class="group">
    <button class="btn small ghost" class:active={fitted} onclick={() => ((fitted = true), fit())}>Fit</button>
    <button class="btn small ghost" onclick={() => setZoom(1)}>1:1</button>
    <span class="mono muted zoom">{Math.round(zoom * 100)} %</span>
  </div>
  <div class="group">
    {#each ["checker", "dark", "light"] as const as bg}
      <button class="swatch swatch-{bg}" class:active={background === bg} onclick={() => (background = bg)} aria-label="{bg} background"></button>
    {/each}
  </div>
  {#if image}
    <span class="mono muted info">{image.width}×{image.height} · {image.format}</span>
  {/if}
</div>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
  class="stage bg-{background}"
  bind:clientWidth={box.width}
  bind:clientHeight={box.height}
  onwheel={wheel}
  onpointerdown={drag}
  ondblclick={() => ((fitted = true), fit())}
>
  <canvas bind:this={canvas} style:width="{box.width}px" style:height="{box.height}px"></canvas>
  {#if loading}
    <div class="overlay muted">Decoding…</div>
  {:else if error}
    <div class="overlay error">{error}</div>
  {/if}
</div>

<style>
  .bar {
    display: flex;
    align-items: center;
    gap: 14px;
    height: 38px;
    padding: 0 12px;
    background: var(--panel);
    border-bottom: 1px solid var(--border);
    flex: none;
  }

  .group {
    display: flex;
    align-items: center;
    gap: 3px;
  }

  .chan {
    width: 24px;
    height: 22px;
    border-radius: 4px;
    border: 1px solid var(--border);
    font-size: 11px;
    font-weight: 700;
    cursor: pointer;
    color: #fff;
  }

  .chan-R { background: #c0392b; }
  .chan-G { background: #27ae60; }
  .chan-B { background: #2f6fd6; }
  .chan-A { background: #7f8790; }

  .chan.off {
    background: transparent;
    color: var(--faint);
  }

  .zoom {
    min-width: 52px;
    text-align: right;
  }

  .swatch {
    width: 18px;
    height: 18px;
    border-radius: 4px;
    border: 1px solid var(--border);
    cursor: pointer;
    padding: 0;
  }

  .swatch.active {
    outline: 2px solid var(--accent);
    outline-offset: 1px;
  }

  .info {
    margin-left: auto;
  }

  .stage {
    flex: 1;
    position: relative;
    overflow: hidden;
    cursor: grab;
  }

  .stage:active {
    cursor: grabbing;
  }

  canvas {
    position: absolute;
    inset: 0;
  }

  .bg-checker,
  .swatch-checker {
    background-color: #9a9a9a;
    background-image:
      linear-gradient(45deg, #6e6e6e 25%, transparent 25%, transparent 75%, #6e6e6e 75%),
      linear-gradient(45deg, #6e6e6e 25%, transparent 25%, transparent 75%, #6e6e6e 75%);
    background-size: 16px 16px;
    background-position: 0 0, 8px 8px;
  }

  .swatch-checker {
    background-size: 8px 8px;
    background-position: 0 0, 4px 4px;
  }

  .bg-dark,
  .swatch-dark {
    background: #0b0c0e;
  }

  .bg-light,
  .swatch-light {
    background: #f4f4f4;
  }

  .overlay {
    position: absolute;
    inset: 0;
    display: grid;
    place-items: center;
    pointer-events: none;
  }

  .error {
    color: var(--accent);
    background: var(--bg);
  }
</style>
