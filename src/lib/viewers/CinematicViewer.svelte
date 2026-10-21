<script lang="ts">
  import { api, type FileRef, type SceneData, type SceneGroup, type SceneTrack } from "../api";
  import { errorMessage } from "../format";
  import { notify, showFile } from "../state.svelte";

  let { mft }: { mft: number } = $props();

  let scene = $state.raw<SceneData | null>(null);
  let error = $state<string | null>(null);
  let tab = $state<"timeline" | "dialogue" | "resources">("timeline");
  let sequence = $state(0);
  let group = $state<number | null>(null);
  let url = $state<string | null>(null);
  let playing = $state<number | null>(null);
  let preparing = $state(false);

  $effect(() => {
    const id = mft;
    scene = null;
    error = null;
    sequence = 0;
    group = null;
    let stale = false;
    api
      .sceneInfo(id)
      .then((s) => {
        if (stale) return;
        scene = s;
        tab = s.sequences.length ? "timeline" : s.texts.length ? "dialogue" : "resources";
      })
      .catch((e) => !stale && (error = errorMessage(e)));
    return () => (stale = true);
  });

  $effect(() => () => {
    if (url) URL.revokeObjectURL(url);
  });

  const current = $derived(scene?.sequences[sequence] ?? null);
  const length = $derived(Math.max(current?.length ?? 1, 0.001));
  const selectedGroup = $derived(current && group !== null ? current.groups[group] : null);
  const spoken = $derived(scene?.texts.filter((t) => t.text) ?? []);

  /** Key times of a group, for its timeline row. */
  function marks(g: SceneGroup) {
    const curve: number[] = [];
    const flags: number[] = [];
    const triggers: number[] = [];
    for (const t of g.tracks) {
      for (const [time] of t.curve) curve.push(time);
      for (const [time] of t.flags) flags.push(time);
      for (const k of t.triggers) triggers.push(k.time);
    }
    return { curve, flags, triggers };
  }

  const x = (time: number) => `${Math.min(100, Math.max(0, (time / length) * 100))}%`;

  /** SVG path of a curve track, scaled into a 100×24 box. */
  function sparkline(t: SceneTrack) {
    if (t.curve.length < 2) return "";
    const values = t.curve.map(([, v]) => v);
    const lo = Math.min(...values);
    const span = Math.max(...values) - lo || 1;
    return t.curve
      .map(([time, v], i) => `${i ? "L" : "M"}${((time / length) * 100).toFixed(2)},${(22 - ((v - lo) / span) * 20).toFixed(2)}`)
      .join(" ");
  }

  const ticks = $derived.by(() => {
    const step = length > 60 ? 10 : length > 20 ? 5 : length > 6 ? 1 : 0.5;
    const list: number[] = [];
    for (let t = 0; t <= length + 1e-6; t += step) list.push(t);
    return list;
  });

  async function play(voice: number) {
    try {
      if (!voice) return;
      preparing = true;
      // The index of voice lines in the sound banks is built once and cached.
      await api.buildVoiceIndex();
      const sound = await api.playVoice(voice);
      if (url) URL.revokeObjectURL(url);
      url = URL.createObjectURL(new Blob([sound.data], { type: sound.mime }));
      playing = voice;
    } catch (e) {
      notify(errorMessage(e), "error");
    } finally {
      preparing = false;
    }
  }

  const rgb = (c: [number, number, number]) => `rgb(${c[0]} ${c[1]} ${c[2]})`;
  const lightNames = ["ground", "sky", "fill", "hemispherical"];
</script>

{#snippet link(f: FileRef | null)}
  {#if f}
    <button class="link mono" title={f.kind ?? "not in archive"} onclick={() => showFile(f.id)}>{f.id}</button>
  {:else}
    <span class="muted">–</span>
  {/if}
{/snippet}

{#if error}
  <div class="message">
    <p class="error">{error}</p>
    <p class="muted small">Older cinematic scene versions lay out their data differently and are not decoded yet.</p>
  </div>
{:else if !scene}
  <div class="message muted">Loading scene…</div>
{:else}
  <div class="bar">
    <span class="tabs">
      <button class="btn small" class:ghost={tab !== "timeline"} onclick={() => (tab = "timeline")} disabled={!scene.sequences.length}>
        Timeline ({scene.sequences.length})
      </button>
      <button class="btn small" class:ghost={tab !== "dialogue"} onclick={() => (tab = "dialogue")}>Dialogue ({spoken.length})</button>
      <button class="btn small" class:ghost={tab !== "resources"} onclick={() => (tab = "resources")}>Resources</button>
    </span>
    {#if tab === "timeline" && scene.sequences.length > 1}
      <select bind:value={sequence} onchange={() => (group = null)}>
        {#each scene.sequences as s, i (i)}
          <option value={i}>{s.name || `sequence ${i + 1}`} ({s.length.toFixed(1)} s){s.name === scene.startingSequence ? " · start" : ""}</option>
        {/each}
      </select>
    {/if}
    {#if preparing}<span class="muted small">Preparing voice lines…</span>{/if}
    {#if url}<audio controls autoplay src={url}></audio>{/if}
  </div>

  {#if tab === "timeline" && current}
    <div class="timeline">
      <div class="meta muted small">
        {current.length.toFixed(2)} s{current.map ? ` · map ${current.map}` : ""}{current.clientMap && current.clientMap !== current.map
          ? ` · client map ${current.clientMap}`
          : ""} · {current.groups.length} track groups · scene version {scene.version}
      </div>
      <div class="scrollrows">
      <div class="ruler">
        <span class="label"></span>
        <span class="lane">
          {#each ticks as t (t)}<span class="tick" style:left={x(t)}>{t}s</span>{/each}
        </span>
      </div>
      <div class="rows">
        {#each current.groups as g, i (i)}
          {@const m = marks(g)}
          <button class="grow" class:active={group === i} onclick={() => (group = group === i ? null : i)}>
            <span class="label"><span class="kind k{g.kind}">{g.kindName}</span> <span class="mono">{g.name}</span></span>
            <span class="lane">
              {#each m.curve as t, j (j)}<span class="dot" style:left={x(t)}></span>{/each}
              {#each m.flags as t, j (j)}<span class="flag" style:left={x(t)}></span>{/each}
              {#each m.triggers as t, j (j)}<span class="trigger" style:left={x(t)}></span>{/each}
            </span>
          </button>
        {/each}
      </div>
      </div>
      <div class="legend muted small">
        <span><i class="dot static"></i>curve key</span><span><i class="flag static"></i>flag key</span><span
          ><i class="trigger static"></i>trigger</span
        >
        <span>Click a group to see its tracks.</span>
      </div>
      {#if selectedGroup}
        <div class="tracks">
          <h3>{selectedGroup.kindName} · <span class="mono">{selectedGroup.name}</span></h3>
          <table>
            <thead><tr><th>Track</th><th>Keys</th><th>Curve</th><th>Triggers</th></tr></thead>
            <tbody>
              {#each selectedGroup.tracks as t, i (i)}
                <tr>
                  <td class="mono small">{t.name || `type ${t.kind}`}</td>
                  <td class="small muted">{t.curve.length} curve · {t.flags.length} flag</td>
                  <td>
                    {#if t.curve.length > 1}
                      <svg viewBox="0 0 100 24" preserveAspectRatio="none" class="spark"><path d={sparkline(t)} /></svg>
                    {:else if t.curve.length === 1}
                      <span class="mono small">{t.curve[0][1].toFixed(3)}</span>
                    {/if}
                  </td>
                  <td class="small">
                    {#each t.triggers as k, j (j)}
                      <span class="trig"><span class="mono">{k.time.toFixed(2)}s</span> {k.token1}{k.token2 ? ` → ${k.token2}` : ""}</span>
                    {/each}
                  </td>
                </tr>
              {/each}
            </tbody>
          </table>
        </div>
      {/if}
    </div>
  {:else if tab === "dialogue"}
    <div class="scroll">
      {#if spoken.length === 0}
        <p class="muted">This scene has no dialogue text.</p>
      {:else}
        <p class="muted small">
          Subtitles of the scene (English). The voice lines play from the sound banks, in the voice language installed with
          the game; building the voice index takes about 20 seconds the first time.
        </p>
        <table>
          <tbody>
            {#each spoken as t, i (i)}
              <tr class:active={playing === t.voice && t.voice !== 0}>
                <td class="play">
                  {#if t.voice}
                    <button class="btn small ghost" onclick={() => play(t.voice)} aria-label="Play" disabled={preparing}>▶</button>
                  {/if}
                </td>
                <td>{t.text}</td>
                <td class="mono small muted">{t.name}</td>
                <td class="mono small muted">{t.voice || ""}</td>
              </tr>
            {/each}
          </tbody>
        </table>
      {/if}
    </div>
  {:else}
    <div class="scroll">
      <dl>
        <dt>Starting sequence</dt>
        <dd class="mono">{scene.startingSequence || "–"}</dd>
        <dt>Scene version</dt>
        <dd>{scene.version}</dd>
      </dl>
      {#if scene.files.length}
        <h3>Files</h3>
        <table>
          <tbody>
            {#each scene.files as [n, f], i (i)}
              <tr><td class="mono">{n}</td><td>{@render link(f)}</td><td class="muted small">{f?.kind ?? ""}</td></tr>
            {/each}
          </tbody>
        </table>
      {/if}
      {#if scene.ambientLights.length}
        <h3>Ambient lights</h3>
        <table>
          <tbody>
            {#each scene.ambientLights as l, i (i)}
              <tr>
                <td class="mono">{l.name}</td>
                {#each l.colors as [intensity, c], j (j)}
                  <td><span class="swatch" style:background={rgb(c)}></span> <span class="small muted">{lightNames[j]} ×{intensity.toFixed(2)}</span></td>
                {/each}
              </tr>
            {/each}
          </tbody>
        </table>
      {/if}
      {#if scene.scripts.length}
        <h3>Scripts</h3>
        <div class="chips">
          {#each scene.scripts as [n, size], i (i)}<span class="chip mono">{n || "–"} <span class="muted">{size} B</span></span>{/each}
        </div>
      {/if}
      {#if scene.species.length}
        <h3>Species</h3>
        <table>
          <tbody>
            {#each scene.species as s, i (i)}
              <tr><td class="mono">{s.name}</td><td class="mono muted">{s.model}</td><td class="mono muted">{s.variant}</td></tr>
            {/each}
          </tbody>
        </table>
      {/if}
    </div>
  {/if}
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

  .tabs {
    display: flex;
    gap: 4px;
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

  audio {
    margin-left: auto;
    height: 32px;
  }

  .small {
    font-size: 11px;
  }

  .timeline {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
    padding: 8px 12px 8px;
  }

  .scrollrows {
    flex: 1;
    min-height: 80px;
    overflow: auto;
  }

  .ruler {
    position: sticky;
    top: 0;
    z-index: 1;
    background: var(--bg);
  }

  .meta {
    margin-bottom: 6px;
  }

  .ruler,
  .grow {
    display: grid;
    grid-template-columns: 220px 1fr;
    gap: 10px;
    align-items: center;
  }

  .ruler {
    height: 20px;
    font-size: 10px;
    color: var(--muted);
  }

  .lane {
    position: relative;
    height: 100%;
  }

  .tick {
    position: absolute;
    top: 0;
    transform: translateX(-50%);
  }

  .grow {
    width: 100%;
    height: 24px;
    padding: 0;
    border: 0;
    border-bottom: 1px solid color-mix(in srgb, var(--border) 50%, transparent);
    background: none;
    color: inherit;
    font: inherit;
    text-align: left;
    cursor: pointer;
  }

  .grow:hover {
    background: var(--row-hover);
  }

  .grow.active {
    background: var(--accent-soft);
  }

  .grow .lane {
    background: linear-gradient(var(--panel-2), var(--panel-2)) center / 100% 2px no-repeat;
  }

  .label {
    overflow: hidden;
    white-space: nowrap;
    text-overflow: ellipsis;
  }

  .kind {
    display: inline-block;
    min-width: 92px;
    font-size: 11px;
    color: var(--muted);
  }

  .k2,
  .k3 {
    color: var(--tone-text);
  }
  .k1 {
    color: var(--tone-sound);
  }
  .k9 {
    color: var(--tone-model);
  }
  .k8,
  .k14 {
    color: var(--tone-image);
  }
  .k12 {
    color: var(--tone-font);
  }

  .dot,
  .flag,
  .trigger {
    position: absolute;
    top: 50%;
    transform: translate(-50%, -50%);
  }

  .dot {
    width: 5px;
    height: 5px;
    border-radius: 50%;
    background: var(--tone-text);
  }

  .flag {
    width: 2px;
    height: 12px;
    background: var(--tone-map);
  }

  .trigger {
    width: 7px;
    height: 7px;
    background: var(--tone-animation);
    transform: translate(-50%, -50%) rotate(45deg);
  }

  .legend {
    display: flex;
    gap: 16px;
    margin: 8px 0;
  }

  .legend span {
    display: flex;
    align-items: center;
    gap: 6px;
  }

  .legend .static {
    position: static;
    transform: none;
    display: inline-block;
  }

  .legend .trigger.static {
    transform: rotate(45deg);
  }

  .tracks {
    flex: none;
    max-height: 42%;
    overflow: auto;
    border-top: 1px solid var(--border);
  }

  h3 {
    margin: 12px 0 6px;
    font-size: 13px;
  }

  table {
    border-collapse: collapse;
    width: 100%;
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

  tr.active {
    background: var(--accent-soft);
  }

  .spark {
    width: 220px;
    height: 24px;
    display: block;
  }

  .spark path {
    fill: none;
    stroke: var(--tone-text);
    stroke-width: 1.2;
    vector-effect: non-scaling-stroke;
  }

  .trig {
    display: inline-block;
    margin-right: 10px;
  }

  .scroll {
    flex: 1;
    overflow: auto;
    padding: 12px 16px 24px;
  }

  .play {
    width: 36px;
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

  .swatch {
    display: inline-block;
    width: 18px;
    height: 12px;
    border-radius: 3px;
    border: 1px solid var(--border);
    vertical-align: middle;
  }

  .link {
    padding: 0;
    border: 0;
    background: none;
    color: var(--tone-text);
    cursor: pointer;
    font-size: inherit;
  }

  .message {
    padding: 24px;
    text-align: center;
  }

  .error {
    color: var(--accent);
  }
</style>
