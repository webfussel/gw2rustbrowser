<script lang="ts">
  import { api, type StarFieldData } from "../api";
  import { errorMessage } from "../format";

  let { mft }: { mft: number } = $props();

  type Projection = "dome" | "panorama";
  /** Which star value colours the stars (2, 4, 5 are of unknown meaning). */
  type ColorBy = "none" | "2" | "4" | "5";

  const STRIDE = 6;
  const HALF_PI = Math.PI / 2;
  const TAU = Math.PI * 2;

  let data = $state.raw<StarFieldData | null>(null);
  let error = $state<string | null>(null);
  let projection = $state<Projection>("dome");
  let colorBy = $state<ColorBy>("none");
  let sizeScale = $state(1);
  let showGrid = $state(true);
  let box = $state({ width: 0, height: 0 });
  let canvas: HTMLCanvasElement | undefined = $state();
  let hovered = $state<{ index: number; x: number; y: number } | null>(null);
  /** Screen position of every star from the last draw, for hit testing. */
  let screen = new Float32Array(0);

  const count = $derived(data ? data.stars.length / STRIDE : 0);

  $effect(() => {
    let stale = false;
    api
      .loadStars(mft)
      .then((d) => !stale && (data = d))
      .catch((e) => !stale && (error = errorMessage(e)));
    return () => (stale = true);
  });

  function place(azimuth: number, elevation: number, w: number, h: number): [number, number] {
    const height = 1 - Math.min(Math.max(elevation, 0), HALF_PI) / HALF_PI;
    if (projection === "dome") {
      const radius = Math.min(w, h) / 2 - 28;
      const r = height * radius;
      return [w / 2 + r * Math.sin(azimuth), h / 2 - r * Math.cos(azimuth)];
    }
    const pad = 28;
    const turn = ((azimuth % TAU) + TAU) % TAU;
    return [pad + (turn / TAU) * (w - 2 * pad), pad + height * (h - 2 * pad)];
  }

  /** Dark blue → teal → yellow, for values 0..1. */
  function ramp(value: number): string {
    const v = Math.min(Math.max(value, 0), 1);
    const stops = [
      [68, 90, 220],
      [40, 190, 170],
      [250, 220, 80],
    ];
    const at = v * (stops.length - 1);
    const i = Math.min(Math.floor(at), stops.length - 2);
    const f = at - i;
    const c = stops[i].map((a, k) => Math.round(a + (stops[i + 1][k] - a) * f));
    return `rgb(${c[0]},${c[1]},${c[2]})`;
  }

  function drawGrid(ctx: CanvasRenderingContext2D, w: number, h: number) {
    ctx.strokeStyle = "rgba(150,170,220,0.18)";
    ctx.fillStyle = "rgba(170,185,225,0.55)";
    ctx.lineWidth = 1;
    ctx.font = "11px sans-serif";
    if (projection === "dome") {
      const radius = Math.min(w, h) / 2 - 28;
      for (const deg of [0, 30, 60]) {
        const r = (1 - deg / 90) * radius;
        ctx.beginPath();
        ctx.arc(w / 2, h / 2, r, 0, TAU);
        ctx.stroke();
        ctx.fillText(`${deg}°`, w / 2 + 4, h / 2 - r + 12);
      }
      for (let deg = 0; deg < 360; deg += 45) {
        const a = (deg * Math.PI) / 180;
        ctx.beginPath();
        ctx.moveTo(w / 2, h / 2);
        ctx.lineTo(w / 2 + radius * Math.sin(a), h / 2 - radius * Math.cos(a));
        ctx.stroke();
        const lx = w / 2 + (radius + 14) * Math.sin(a);
        const ly = h / 2 - (radius + 14) * Math.cos(a);
        ctx.fillText(`${deg}°`, lx - 10, ly + 4);
      }
    } else {
      const pad = 28;
      for (const deg of [0, 30, 60, 90]) {
        const y = pad + (1 - deg / 90) * (h - 2 * pad);
        ctx.beginPath();
        ctx.moveTo(pad, y);
        ctx.lineTo(w - pad, y);
        ctx.stroke();
        ctx.fillText(`${deg}°`, 2, y + 4);
      }
      for (let deg = 0; deg <= 360; deg += 45) {
        const x = pad + (deg / 360) * (w - 2 * pad);
        ctx.beginPath();
        ctx.moveTo(x, pad);
        ctx.lineTo(x, h - pad);
        ctx.stroke();
        ctx.fillText(`${deg}°`, x - 10, h - 8);
      }
    }
  }

  $effect(() => {
    if (!canvas || !data || !box.width || !box.height) return;
    const { width: w, height: h } = box;
    const dpr = window.devicePixelRatio || 1;
    canvas.width = Math.round(w * dpr);
    canvas.height = Math.round(h * dpr);
    const ctx = canvas.getContext("2d")!;
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);

    const sky = ctx.createRadialGradient(w / 2, h / 2, 0, w / 2, h / 2, Math.max(w, h) / 1.4);
    sky.addColorStop(0, "#111a33");
    sky.addColorStop(1, "#05070f");
    ctx.fillStyle = sky;
    ctx.fillRect(0, 0, w, h);
    if (showGrid) drawGrid(ctx, w, h);

    const s = data.stars;
    const n = s.length / STRIDE;
    screen = new Float32Array(n * 2);
    const channel = colorBy === "none" ? -1 : Number(colorBy);
    for (let i = 0; i < n; i++) {
      const o = i * STRIDE;
      const brightness = Math.min(Math.max(s[o + 3], 0), 1);
      const [x, y] = place(s[o], s[o + 1], w, h);
      screen[i * 2] = x;
      screen[i * 2 + 1] = y;
      const radius = (0.6 + brightness * 2.2) * sizeScale;
      const color = channel < 0 ? "rgb(235,240,255)" : ramp(s[o + channel]);
      if (brightness > 0.55) {
        // Soft glow around the brighter stars.
        ctx.globalAlpha = 0.18 * brightness;
        ctx.fillStyle = color;
        ctx.beginPath();
        ctx.arc(x, y, radius * 3, 0, TAU);
        ctx.fill();
      }
      ctx.globalAlpha = 0.35 + 0.65 * brightness;
      ctx.fillStyle = color;
      ctx.beginPath();
      ctx.arc(x, y, radius, 0, TAU);
      ctx.fill();
    }
    ctx.globalAlpha = 1;

    if (hovered) {
      ctx.strokeStyle = "#e5483d";
      ctx.lineWidth = 2;
      ctx.beginPath();
      ctx.arc(screen[hovered.index * 2], screen[hovered.index * 2 + 1], 7, 0, TAU);
      ctx.stroke();
    }
  });

  function hover(event: MouseEvent) {
    const rect = (event.currentTarget as HTMLElement).getBoundingClientRect();
    const mx = event.clientX - rect.left;
    const my = event.clientY - rect.top;
    let best = -1;
    let bestDistance = 64; // 8 px radius, squared
    for (let i = 0; i < screen.length / 2; i++) {
      const dx = screen[i * 2] - mx;
      const dy = screen[i * 2 + 1] - my;
      const d = dx * dx + dy * dy;
      if (d < bestDistance) {
        bestDistance = d;
        best = i;
      }
    }
    hovered = best < 0 ? null : { index: best, x: mx, y: my };
  }

  const degrees = (radians: number) => `${((radians * 180) / Math.PI).toFixed(1)}°`;
  const star = $derived(hovered && data ? data.stars.slice(hovered.index * STRIDE, hovered.index * STRIDE + STRIDE) : null);
</script>

<div class="bar">
  <div class="group">
    <button class="btn small ghost" class:active={projection === "dome"} onclick={() => (projection = "dome")}>Dome</button>
    <button class="btn small ghost" class:active={projection === "panorama"} onclick={() => (projection = "panorama")}>Panorama</button>
  </div>
  <label>
    Color by
    <select bind:value={colorBy}>
      <option value="none">nothing</option>
      <option value="2">value 2 (unknown)</option>
      <option value="4">value 4 (unknown)</option>
      <option value="5">value 5 (unknown)</option>
    </select>
  </label>
  <label>Star size <input type="range" min="0.5" max="3" step="0.1" bind:value={sizeScale} /></label>
  <label><input type="checkbox" bind:checked={showGrid} /> Grid</label>
  {#if data}
    <span class="mono muted info">{count.toLocaleString()} stars · header {data.header}</span>
  {/if}
</div>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
  class="stage"
  bind:clientWidth={box.width}
  bind:clientHeight={box.height}
  onmousemove={hover}
  onmouseleave={() => (hovered = null)}
>
  <canvas bind:this={canvas} style:width="{box.width}px" style:height="{box.height}px"></canvas>
  {#if colorBy !== "none"}
    <div class="legend">
      <span>0</span><span class="ramp"></span><span>1</span><span class="muted">value {colorBy}</span>
    </div>
  {/if}
  {#if star && hovered}
    <div class="tip" style:left="{Math.min(hovered.x + 14, box.width - 190)}px" style:top="{Math.min(hovered.y + 14, box.height - 140)}px">
      <div class="tip-title">Star {hovered.index + 1}</div>
      <dl>
        <dt>Azimuth</dt><dd>{degrees(star[0])}</dd>
        <dt>Elevation</dt><dd>{degrees(star[1])}</dd>
        <dt>Brightness</dt><dd>{star[3].toFixed(3)}</dd>
        <dt>Value 2</dt><dd>{star[2].toFixed(3)}</dd>
        <dt>Value 4</dt><dd>{star[4].toFixed(3)}</dd>
        <dt>Value 5</dt><dd>{star[5].toFixed(3)}</dd>
      </dl>
    </div>
  {/if}
  {#if error}
    <div class="overlay error">{error}</div>
  {:else if !data}
    <div class="overlay muted">Loading…</div>
  {/if}
</div>

<style>
  .bar {
    display: flex;
    align-items: center;
    gap: 16px;
    min-height: 40px;
    padding: 4px 12px;
    background: var(--panel);
    border-bottom: 1px solid var(--border);
    flex: none;
    flex-wrap: wrap;
  }

  .group {
    display: flex;
    gap: 2px;
  }

  label {
    display: flex;
    align-items: center;
    gap: 6px;
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

  input[type="range"] {
    accent-color: var(--accent);
    width: 100px;
  }

  .info {
    margin-left: auto;
  }

  .stage {
    flex: 1;
    position: relative;
    overflow: hidden;
    background: #05070f;
  }

  canvas {
    position: absolute;
    inset: 0;
  }

  .legend {
    position: absolute;
    right: 12px;
    bottom: 12px;
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 4px 8px;
    border-radius: 6px;
    background: rgba(10, 12, 20, 0.75);
    color: #dfe4f2;
    font-size: 11px;
  }

  .ramp {
    width: 100px;
    height: 8px;
    border-radius: 4px;
    background: linear-gradient(90deg, rgb(68, 90, 220), rgb(40, 190, 170), rgb(250, 220, 80));
  }

  .tip {
    position: absolute;
    width: 176px;
    padding: 8px 10px;
    border-radius: 8px;
    border: 1px solid var(--border);
    background: var(--panel);
    box-shadow: 0 6px 20px rgba(0, 0, 0, 0.4);
    pointer-events: none;
    font-size: 12px;
  }

  .tip-title {
    font-weight: 600;
    margin-bottom: 4px;
  }

  dl {
    display: grid;
    grid-template-columns: max-content 1fr;
    gap: 2px 10px;
    margin: 0;
  }

  dt {
    color: var(--muted);
  }

  dd {
    margin: 0;
    font-family: var(--mono);
    text-align: right;
  }

  .overlay {
    position: absolute;
    inset: 0;
    display: grid;
    place-items: center;
  }

  .error {
    color: var(--accent);
  }
</style>
