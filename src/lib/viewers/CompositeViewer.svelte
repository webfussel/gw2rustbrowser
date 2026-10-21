<script lang="ts">
  import { api, type CompositeColor, type CompositeInfo, type CompositeRace, type FileRef } from "../api";
  import { errorMessage } from "../format";
  import { showFile } from "../state.svelte";
  import ImageViewer from "./ImageViewer.svelte";

  let { mft }: { mft: number } = $props();

  const ROW = 28;
  const modelViewer = () => import("./ModelViewer.svelte");

  let info = $state.raw<CompositeInfo | null>(null);
  let error = $state<string | null>(null);
  /** Selected race; null shows the file overview. */
  let selected = $state<number | null>(null);
  let race = $state.raw<CompositeRace | null>(null);
  let raceError = $state<string | null>(null);
  let tab = $state<"overview" | "parts" | "variants">("overview");
  let filter = $state("");
  let part = $state<number | null>(null);
  let scrollTop = $state(0);
  let viewport = $state(300);

  $effect(() => {
    const id = mft;
    info = null;
    error = null;
    selected = null;
    let stale = false;
    api
      .compositeInfo(id)
      .then((c) => !stale && (info = c))
      .catch((e) => !stale && (error = errorMessage(e)));
    return () => (stale = true);
  });

  $effect(() => {
    const index = selected;
    race = null;
    raceError = null;
    part = null;
    filter = "";
    scrollTop = 0;
    if (index === null) return;
    let stale = false;
    api
      .compositeRace(mft, index)
      .then((r) => !stale && (race = r))
      .catch((e) => !stale && (raceError = errorMessage(e)));
    return () => (stale = true);
  });

  const parts = $derived.by(() => {
    const needle = filter.trim().toLowerCase();
    const all = (race?.parts ?? []).map((p, i) => ({ p, i }));
    return needle ? all.filter(({ p }) => p.name.includes(needle) || String(p.mesh?.id ?? "").startsWith(needle)) : all;
  });
  const first = $derived(Math.max(0, Math.floor(scrollTop / ROW) - 10));
  const visible = $derived(parts.slice(first, first + Math.ceil(viewport / ROW) + 20));
  const current = $derived(race && part !== null ? race.parts[part] : null);

  /** Colour adjustment as text: brightness, contrast, hue, saturation, lightness. */
  const adjustment = (c: CompositeColor) => c.join(" ");
  const NONE = 4294967295;
  const colorNames = ["eye", "hair", "hair 2", "pattern", "skin"];
  const patternSlots = ["chest", "face", "feet", "hands", "legs", "ears"];
</script>

{#snippet link(f: FileRef | null)}
  {#if f}
    <button class="link mono" title={f.kind ?? "not in archive"} onclick={() => showFile(f.id)}>{f.id}</button>
  {:else}
    <span class="muted">–</span>
  {/if}
{/snippet}

{#snippet chips(title: string, list: string[])}
  {#if list.length}
    <h3>{title} <span class="muted small">{list.length}</span></h3>
    <div class="chips">{#each list as c, i (i)}<span class="chip mono">{c || "–"}</span>{/each}</div>
  {/if}
{/snippet}

{#if error}
  <div class="message error">{error}</div>
{:else if !info}
  <div class="message muted">Loading character data…</div>
{:else}
  <div class="layout">
    <aside>
      <button class="row" class:active={selected === null} onclick={() => (selected = null)}>Overview</button>
      <div class="heading muted">{info.races.length} races and sexes</div>
      <div class="list">
        {#each info.races as r, i (i)}
          <button class="row" class:active={selected === i} onclick={() => ((selected = i), (tab = "overview"))}>
            <span>{r.name}</span>
            <span class="muted small">{r.parts.toLocaleString()} parts · {r.variants} variants</span>
          </button>
        {/each}
      </div>
    </aside>

    <div class="detail">
      {#if selected === null}
        <div class="scroll">
          <h2>Character composition</h2>
          <p class="muted">
            How player characters and humanoid NPCs are put together: per race and sex the skeleton, faces, hair styles,
            body shapes, skin patterns, preset variants and every body and armour part with its mesh, dye masks and
            textures. Chunk version {info.version}, config version {info.configVersion}, {info.boneScales} shared bone scales.
          </p>
          <h3>Texture atlas layouts</h3>
          <p class="muted small">Where each part is drawn into a character's composed armour texture.</p>
          <div class="atlases">
            {#each info.blitRects as b (b.name)}
              <figure>
                <svg viewBox="0 0 {b.size[0]} {b.size[1]}">
                  <rect width={b.size[0]} height={b.size[1]} class="sheet" />
                  {#each b.rects as [x0, y0, x1, y1], i (i)}
                    <rect x={x0} y={y0} width={x1 - x0} height={y1 - y0} class="cell"
                      ><title>rect {i}: {x0}, {y0} – {x1}, {y1}</title></rect
                    >
                    <text x={x0 + 6} y={y0 + 26} class="label">{i}</text>
                  {/each}
                </svg>
                <figcaption>{b.name} <span class="muted small">{b.size[0]}×{b.size[1]} · {b.rects.length} rects</span></figcaption>
              </figure>
            {/each}
          </div>
        </div>
      {:else if raceError}
        <div class="message error">{raceError}</div>
      {:else if !race}
        <div class="message muted">Loading…</div>
      {:else}
        <div class="bar">
          <strong>{race.name}</strong>
          <span class="tabs">
            <button class="btn small" class:ghost={tab !== "overview"} onclick={() => (tab = "overview")}>Overview</button>
            <button class="btn small" class:ghost={tab !== "parts"} onclick={() => (tab = "parts")}>Parts ({race.parts.length.toLocaleString()})</button>
            <button class="btn small" class:ghost={tab !== "variants"} onclick={() => (tab = "variants")}>Variants ({race.variants.length})</button>
          </span>
          {#if tab === "parts"}
            <input type="search" placeholder="Filter by name or mesh id" bind:value={filter} />
          {/if}
        </div>

        {#if tab === "overview"}
          <div class="scroll">
            <dl>
              <dt>Skeleton / animations</dt>
              <dd>{@render link(race.skeleton)} <span class="muted small">{race.skeleton?.kind ?? ""}</span></dd>
              <dt>Colour palettes</dt>
              <dd>eye <span class="mono">{race.palettes[0]}</span> · hair <span class="mono">{race.palettes[1]}</span> · skin <span class="mono">{race.palettes[2]}</span></dd>
              {#if race.variantOf}<dt>Variant of</dt><dd class="mono">{race.variantOf}</dd>{/if}
              <dt>Face proportions</dt>
              <dd>{race.faceBoneScales} bone scale sets</dd>
            </dl>
            {@render chips("Body shapes", race.bodyShapes)}
            {@render chips("Faces", race.faces)}
            {@render chips("Hair styles", race.hairStyles)}
            {@render chips("Beards", race.beards)}
            {@render chips("Ears", race.ears)}
            {#if race.skinPatterns.length}
              <h3>Skin patterns <span class="muted small">{race.skinPatterns.length}</span></h3>
              <table>
                <thead><tr><th>#</th>{#each patternSlots as s (s)}<th>{s}</th>{/each}</tr></thead>
                <tbody>
                  {#each race.skinPatterns as p, i (i)}
                    <tr><td class="muted">{i}</td>{#each p as f, j (j)}<td>{@render link(f)}</td>{/each}</tr>
                  {/each}
                </tbody>
              </table>
            {/if}
            {#if race.animOverrides.length}
              <h3>Animation overrides <span class="muted small">{race.animOverrides.length}</span></h3>
              <div class="overrides">
                {#each race.animOverrides as [role, f], i (i)}
                  <span class="override"><span class="mono">{role || "–"}</span> {@render link(f)}</span>
                {/each}
              </div>
            {/if}
          </div>
        {:else if tab === "parts"}
          <div class="parts">
            <div class="prow head"><span>Part</span><span>Slot</span><span>Mesh</span><span>Texture</span><span>Normal</span><span>Dye masks</span></div>
            <div class="list" bind:clientHeight={viewport} onscroll={(e) => (scrollTop = e.currentTarget.scrollTop)}>
              <div class="spacer" style:height="{parts.length * ROW}px">
                {#each visible as { p, i }, j (i)}
                  <div
                    class="prow"
                    class:active={part === i}
                    style:top="{(first + j) * ROW}px"
                    role="button"
                    tabindex="0"
                    onclick={() => (part = part === i ? null : i)}
                    onkeydown={(e) => e.key === "Enter" && (part = part === i ? null : i)}
                  >
                    <span class="mono">{p.name}</span>
                    <span class="muted">{p.kind}</span>
                    <span>{@render link(p.mesh)}</span>
                    <span>{@render link(p.texture)}</span>
                    <span>{@render link(p.normal)}</span>
                    <span class="masks">{#each p.dyeMasks as m, j (j)}{@render link(m)}{/each}</span>
                  </div>
                {/each}
              </div>
            </div>
            {#if current}
              <div class="partview">
                <div class="pane">
                  {#if current.mesh && current.mesh.mft >= 0}
                    {#key current.mesh.mft}
                      {#await modelViewer() then { default: ModelViewer }}
                        <ModelViewer mft={current.mesh.mft} />
                      {/await}
                    {/key}
                  {:else}
                    <div class="message muted">No mesh</div>
                  {/if}
                </div>
                <div class="pane">
                  {#if current.texture}
                    {#key current.texture.id}
                      <ImageViewer load={() => api.loadTexture(current!.texture!.id)} />
                    {/key}
                  {:else}
                    <div class="message muted">No texture</div>
                  {/if}
                </div>
              </div>
            {:else}
              <p class="muted small hint">Click a part to see its mesh and texture.</p>
            {/if}
          </div>
        {:else}
          <div class="scroll">
            <p class="muted small">
              Colours are adjustments applied to the base textures, like dyes: brightness, contrast, hue, saturation and
              lightness (0–255 each), not absolute colours.
            </p>
            <table>
              <thead><tr><th>Variant</th>{#each colorNames as c (c)}<th>{c}</th>{/each}<th>Components</th><th>Body shape</th></tr></thead>
              <tbody>
                {#each race.variants as v, i (i)}
                  <tr>
                    <td class="mono">{v.name || "–"}</td>
                    {#each v.colors as c, j (j)}<td class="mono small">{adjustment(c)}</td>{/each}
                    <td class="small">
                      {#each v.components as [n, cs] (n)}
                        <span class="component" title={cs.map(adjustment).join(" | ")}><span class="mono">{n}</span></span>
                      {/each}
                    </td>
                    <td class="muted">{v.boneScale === NONE ? "–" : v.boneScale}</td>
                  </tr>
                {/each}
              </tbody>
            </table>
          </div>
        {/if}
      {/if}
    </div>
  </div>
{/if}

<style>
  .layout {
    flex: 1;
    display: flex;
    min-height: 0;
  }

  aside {
    width: 240px;
    flex: none;
    display: flex;
    flex-direction: column;
    background: var(--panel);
    border-right: 1px solid var(--border);
  }

  .heading {
    padding: 10px 12px 4px;
    font-size: 11px;
    text-transform: uppercase;
    letter-spacing: 0.04em;
  }

  aside .list {
    flex: 1;
    overflow: auto;
  }

  .row {
    width: 100%;
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 1px;
    padding: 6px 12px;
    border: 0;
    background: none;
    color: inherit;
    font: inherit;
    cursor: pointer;
    text-align: left;
  }

  .row:hover {
    background: var(--row-hover);
  }

  .row.active {
    background: var(--accent-soft);
    box-shadow: inset 2px 0 var(--accent);
  }

  .small {
    font-size: 11px;
  }

  .detail {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
  }

  .scroll {
    flex: 1;
    overflow: auto;
    padding: 14px 18px 24px;
  }

  h2 {
    margin: 0 0 6px;
    font-size: 18px;
  }

  h3 {
    margin: 16px 0 6px;
    font-size: 13px;
  }

  .bar {
    display: flex;
    align-items: center;
    gap: 12px;
    min-height: 40px;
    padding: 4px 12px;
    background: var(--panel);
    border-bottom: 1px solid var(--border);
    flex: none;
    flex-wrap: wrap;
  }

  .tabs {
    display: flex;
    gap: 4px;
  }

  .bar input {
    width: 220px;
  }

  dl {
    display: grid;
    grid-template-columns: max-content 1fr;
    gap: 4px 16px;
    margin: 0;
  }

  dt {
    color: var(--muted);
  }

  dd {
    margin: 0;
  }

  .chips {
    display: flex;
    flex-wrap: wrap;
    gap: 4px;
  }

  .chip {
    padding: 1px 8px;
    border: 1px solid var(--border);
    border-radius: 999px;
    font-size: 11px;
  }

  .overrides {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(200px, 1fr));
    gap: 2px 16px;
  }

  .override {
    display: flex;
    justify-content: space-between;
    gap: 8px;
  }

  table {
    border-collapse: collapse;
  }

  th {
    text-align: left;
    font-weight: 500;
    color: var(--muted);
    font-size: 11px;
    padding: 3px 12px 3px 0;
    border-bottom: 1px solid var(--border);
  }

  td {
    padding: 3px 12px 3px 0;
    border-bottom: 1px solid var(--border);
    vertical-align: middle;
  }

  .component {
    display: inline-flex;
    align-items: center;
    gap: 2px;
    margin-right: 10px;
  }

  .parts {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
  }

  .parts .list {
    flex: 1;
    min-height: 120px;
    overflow: auto;
  }

  .spacer {
    position: relative;
  }

  .prow {
    display: grid;
    grid-template-columns: minmax(140px, 1fr) 50px 90px 90px 90px 2fr;
    gap: 10px;
    align-items: center;
    height: 28px;
    padding: 0 12px;
    border-bottom: 1px solid color-mix(in srgb, var(--border) 50%, transparent);
    cursor: pointer;
  }

  .list .prow {
    position: absolute;
    left: 0;
    right: 0;
  }

  .prow.head {
    flex: none;
    cursor: default;
    color: var(--muted);
    font-size: 11px;
    background: var(--panel-2);
  }

  .list .prow:hover {
    background: var(--row-hover);
  }

  .prow.active {
    background: var(--accent-soft);
  }

  .masks {
    display: flex;
    gap: 8px;
  }

  .partview {
    flex: 1.3;
    min-height: 0;
    display: grid;
    grid-template-columns: 1fr 1fr;
    border-top: 1px solid var(--border);
  }

  .pane {
    min-width: 0;
    min-height: 0;
    display: flex;
    flex-direction: column;
    border-right: 1px solid var(--border);
  }

  .hint {
    margin: 6px 12px;
  }

  .atlases {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(220px, 1fr));
    gap: 16px;
  }

  figure {
    margin: 0;
  }

  svg {
    width: 100%;
    aspect-ratio: 1;
    display: block;
  }

  .sheet {
    fill: var(--panel-2);
    stroke: var(--border);
    stroke-width: 4;
  }

  .cell {
    fill: color-mix(in srgb, var(--tone-model) 25%, transparent);
    stroke: var(--tone-model);
    stroke-width: 3;
  }

  .label {
    fill: var(--text);
    font-size: 26px;
    font-family: var(--mono);
  }

  figcaption {
    margin-top: 4px;
  }

  .link {
    padding: 0;
    border: 0;
    background: none;
    color: var(--tone-text);
    cursor: pointer;
    font-size: inherit;
  }

  .link:hover {
    text-decoration: underline;
  }

  .message {
    padding: 24px;
    text-align: center;
  }

  .error {
    color: var(--accent);
  }
</style>
