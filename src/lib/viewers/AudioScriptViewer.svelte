<script lang="ts">
  import { api, type AudioScriptInfo, type ScriptFile, type ScriptSoundFile } from "../api";
  import { errorMessage } from "../format";
  import { notify } from "../state.svelte";

  let { mft }: { mft: number } = $props();

  type Tab = "sounds" | "music" | "triggers" | "handlers" | "references" | "properties" | "settings";

  let info = $state.raw<AudioScriptInfo | null>(null);
  let error = $state<string | null>(null);
  let tab = $state<Tab>("sounds");
  let filter = $state("");
  let open = $state<string | null>(null);
  let url = $state<string | null>(null);
  let playing = $state<string | null>(null);

  $effect(() => {
    const id = mft;
    info = null;
    error = null;
    open = null;
    filter = "";
    let stale = false;
    api
      .audioScriptInfo(id)
      .then((s) => {
        if (stale) return;
        info = s;
        tab = s.settings ? "music" : s.sounds.length ? "sounds" : s.triggers.length ? "triggers" : "sounds";
      })
      .catch((e) => !stale && (error = errorMessage(e)));
    return () => (stale = true);
  });

  $effect(() => () => {
    if (url) URL.revokeObjectURL(url);
  });

  const tabs = $derived.by(() => {
    if (!info) return [];
    const list: [Tab, string, number | null][] = [
      ["sounds", "Sounds", info.sounds.length],
      ["triggers", "Triggers", info.triggers.length],
      ["handlers", "Handlers", info.handlers.length],
      ["references", "Script references", info.scriptRefs.length],
      ["properties", "Properties", info.properties.length],
    ];
    if (info.settings) {
      list.unshift(["music", "Music playlists", info.settings.playlists.length]);
      list.push(["settings", "Audio settings", null]);
    }
    // Hide empty sections, but always show at least the sound list.
    const shown = list.filter(([, , n]) => n !== 0);
    return shown.length ? shown : list.filter(([key]) => key === "sounds");
  });

  const needle = $derived(filter.trim().toLowerCase());
  const sounds = $derived(
    info?.sounds.filter((s) => !needle || s.name.includes(needle) || (s.category ?? "").includes(needle)) ?? [],
  );
  const playlists = $derived(
    info?.settings?.playlists.filter(
      (p) => !needle || [p.name, p.category, p.primary, p.secondary].some((v) => (v ?? "").includes(needle)),
    ) ?? [],
  );

  async function play(file: ScriptFile, key: string) {
    if (file.mft < 0) return;
    try {
      const sound = await api.loadSound(file.mft);
      if (url) URL.revokeObjectURL(url);
      url = URL.createObjectURL(new Blob([sound.data], { type: sound.mime }));
      playing = key;
    } catch (e) {
      notify(errorMessage(e), "error");
    }
  }

  const toggle = (key: string) => (open = open === key ? null : key);
  const hex = (n: number) => `0x${n.toString(16)}`;
  const num = (n: number) => String(Math.round(n * 1000) / 1000);
</script>

{#snippet files(list: ScriptSoundFile[], prefix: string)}
  <div class="files">
    {#each list as f, i (i)}
      {@const key = `${prefix}:${i}`}
      <div class="file" class:active={playing === key}>
        {#if f.file && f.file.mft >= 0}
          <button class="btn small ghost" onclick={() => play(f.file!, key)} aria-label="Play">▶</button>
        {:else}
          <span class="placeholder"></span>
        {/if}
        <span class="mono">{f.file ? f.file.id : "no file"}</span>
        <span class="muted">{f.file ? (f.file.kind ?? "not in archive") : ""}</span>
        <span class="muted">volume {num(f.volume)} · weight {num(f.weight)}</span>
        {#if f.condition}<span class="muted">if {f.condition}</span>{/if}
        {#if f.language}<span class="muted">language {f.language}</span>{/if}
      </div>
    {/each}
  </div>
{/snippet}

{#if error}
  <div class="message error">{error}</div>
{:else if !info}
  <div class="message muted">Loading audio script…</div>
{:else}
  <div class="bar">
    <span class="tabs">
      {#each tabs as [key, label, n] (key)}
        <button class="btn small" class:ghost={tab !== key} onclick={() => ((tab = key), (open = null))}>
          {label}{n !== null ? ` (${n.toLocaleString()})` : ""}
        </button>
      {/each}
    </span>
    {#if tab === "sounds" || tab === "music"}
      <input type="search" placeholder="Filter by name or category" bind:value={filter} />
    {/if}
    {#if url}
      <audio controls autoplay src={url}></audio>
    {/if}
  </div>

  <div class="body">
    <p class="summary muted">
      AMSP v{info.version} · volume {num(info.volume)} · sound pool {info.soundPoolCount}{info.musicCue
        ? ` · music cue ${info.musicCue}`
        : ""}{info.snapshot ? ` · snapshot ${info.snapshot}` : ""}{info.reverbOverride ? ` · reverb ${info.reverbOverride}` : ""}
    </p>

    {#if tab === "sounds"}
      {#if sounds.length === 0}
        <p class="muted">{info.sounds.length ? "No sound matches the filter." : "This script defines no sounds."}</p>
      {/if}
      {#each sounds as s, i (i)}
        {@const key = `s${i}`}
        <div class="item" class:expanded={open === key}>
          <button class="head" onclick={() => toggle(key)}>
            <span class="mono name">{s.name}</span>
            <span class="tag">{s.category ?? "–"}</span>
            <span class="muted">{s.files.length} {s.files.length === 1 ? "file" : "files"}</span>
            <span class="muted">volume {s.volume}</span>
            <span class="muted">pitch {s.pitch}</span>
            {#if s.loopCount !== 1}<span class="muted">loops {s.loopCount || "∞"}</span>{/if}
          </button>
          {#if open === key}
            <div class="detail">
              <dl>
                <dt>Fade</dt>
                <dd>in {num(s.fadeIn)} s · out {num(s.fadeOut)} s</dd>
                <dt>Pan</dt>
                <dd>{s.pan}</dd>
                {#if s.initialDelay}<dt>Initial delay</dt><dd>{s.initialDelay} s</dd>{/if}
                {#if s.playLength}<dt>Play length</dt><dd>{s.playLength} s</dd>{/if}
                {#if s.repeatTime || (s.repeatCount && s.repeatCount !== "1")}
                  <dt>Repeat</dt>
                  <dd>{s.repeatCount ?? "–"} times, every {s.repeatTime ?? "–"} s</dd>
                {/if}
                {#if s.endCue}<dt>End cue</dt><dd class="mono">{s.endCue}</dd>{/if}
                {#if s.offsetBone}<dt>Bone</dt><dd class="mono">{s.offsetBone}</dd>{/if}
                {#if s.playlist}<dt>Playlist</dt><dd class="mono">{s.playlist}</dd>{/if}
                <dt>Playback</dt>
                <dd>
                  mode {s.playbackMode} · position mode {s.positionMode} · file order {s.fileIterateMode} · loop mode {s.loopMode}
                  · priority {s.priority}{s.channelMax ? ` · max ${s.channelMax} channels` : ""}
                </dd>
                <dt>Effects</dt>
                <dd>
                  {s.dspEffects} DSP effects{s.attenuation ? " · distance attenuation" : ""} · flags {hex(s.flags)}
                </dd>
              </dl>
              {@render files(s.files, key)}
            </div>
          {/if}
        </div>
      {/each}
    {:else if tab === "music" && info.settings}
      {#each playlists as p, i (i)}
        {@const key = `p${i}`}
        <div class="item" class:expanded={open === key}>
          <button class="head" onclick={() => toggle(key)}>
            <span class="mono name">{p.name}</span>
            <span class="tag">{p.primary ?? p.category ?? "–"}</span>
            {#if p.secondary}<span class="tag">{p.secondary}</span>{/if}
            <span class="muted">{p.files.length} {p.files.length === 1 ? "track" : "tracks"}</span>
            <span class="muted">volume {p.volume}</span>
          </button>
          {#if open === key}
            <div class="detail">
              <dl>
                <dt>Category</dt>
                <dd>{p.category ?? "–"}</dd>
                <dt>Fade</dt>
                <dd>in {num(p.fadeIn)} s · out {num(p.fadeOut)} s</dd>
                {#if p.initialSilence}<dt>Initial silence</dt><dd>{p.initialSilence} s</dd>{/if}
                {#if p.intervalSilence}<dt>Silence between tracks</dt><dd>{p.intervalSilence} s</dd>{/if}
                <dt>Track order</dt>
                <dd>{p.fileIterateMode} · flags {hex(p.flags)}</dd>
              </dl>
              {@render files(p.files, key)}
            </div>
          {/if}
        </div>
      {/each}
    {:else if tab === "triggers"}
      {#each info.triggers as t, i (i)}
        <section>
          <h3 class="mono">{t.name}</h3>
          <table>
            <colgroup><col style="width: 90px" /><col style="width: 40%" /><col style="width: 40%" /><col /></colgroup>
            <thead><tr><th>Time</th><th>Cue</th><th>End cue</th><th>Type</th></tr></thead>
            <tbody>
              {#each t.markers as m, j (j)}
                <tr>
                  <td class="mono">{num(m.time)} s</td>
                  <td class="mono">{m.cue ?? "–"}</td>
                  <td class="mono muted">{m.end ?? "–"}</td>
                  <td class="muted">{m.kind}</td>
                </tr>
              {/each}
            </tbody>
          </table>
        </section>
      {/each}
    {:else if tab === "handlers"}
      {#each info.handlers as h, i (i)}
        {@const key = `h${i}`}
        <div class="item" class:expanded={open === key}>
          <button class="head" onclick={() => toggle(key)}>
            <span class="mono name">{h.name}</span>
            <span class="muted">{h.size} bytes of byte code</span>
            {#if h.flags}<span class="muted">flags {hex(h.flags)}</span>{/if}
          </button>
          {#if open === key}
            <pre class="code">{h.code}{h.size > 1024 ? " …" : ""}</pre>
          {/if}
        </div>
      {/each}
    {:else if tab === "references"}
      <table>
        <tbody>
          {#each info.scriptRefs as r, i (i)}
            <tr>
              <td class="mono">{r.name}</td>
              <td class="mono">{r.file?.id ?? "–"}</td>
              <td class="muted">{r.file ? (r.file.kind ?? "not in archive") : ""}</td>
            </tr>
          {/each}
        </tbody>
      </table>
    {:else if tab === "properties"}
      <table>
        <tbody>
          {#each info.properties as p, i (i)}
            <tr>
              <td class="mono">{p.name}</td>
              <td class="mono">{p.token ?? ""}</td>
              <td class="mono muted">{num(p.value)}</td>
            </tr>
          {/each}
        </tbody>
      </table>
    {:else if tab === "settings" && info.settings}
      {@const st = info.settings}
      <dl>
        <dt>Default snapshot</dt>
        <dd class="mono">{st.defaultSnapshot ?? "–"}</dd>
        <dt>Distance scale</dt>
        <dd>{num(st.distanceScale)} units per metre</dd>
        <dt>Doppler scale</dt>
        <dd>{num(st.dopplerScale)}</dd>
        <dt>Definitions</dt>
        <dd>
          {st.busses} busses · {st.categories} categories · {st.materials} materials · {st.reverbs} reverbs · {st.snapshots}
          snapshots (not decoded further)
        </dd>
        <dt>Sound bank index</dt>
        <dd class="mono">{st.bankIndex?.id ?? "–"}</dd>
        <dt>Bank script</dt>
        <dd class="mono">{st.bankScript?.id ?? "–"}</dd>
        <dt>Music script</dt>
        <dd class="mono">{st.musicScript?.id ?? "–"}</dd>
      </dl>
      {#if st.musicExternal.length}
        <h3>External music playlists</h3>
        <table>
          <tbody>
            {#each st.musicExternal as [name, playlist] (name)}
              <tr><td class="mono">{name}</td><td>{playlist}</td></tr>
            {/each}
          </tbody>
        </table>
      {/if}
    {/if}
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

  .tabs {
    display: flex;
    gap: 4px;
    flex-wrap: wrap;
  }

  .bar input {
    width: 220px;
  }

  audio {
    margin-left: auto;
    height: 32px;
  }

  .body {
    flex: 1;
    overflow: auto;
    padding: 12px 16px 24px;
  }

  .summary {
    margin: 0 0 12px;
  }

  .item {
    border-bottom: 1px solid var(--border);
  }

  .item.expanded {
    background: var(--panel);
  }

  .head {
    width: 100%;
    display: flex;
    align-items: baseline;
    gap: 14px;
    padding: 6px 4px;
    border: 0;
    background: none;
    color: inherit;
    font: inherit;
    text-align: left;
    cursor: pointer;
    flex-wrap: wrap;
  }

  .head:hover {
    background: var(--row-hover);
  }

  .name {
    min-width: 160px;
  }

  .tag {
    padding: 0 8px;
    border: 1px solid var(--border);
    border-radius: 999px;
    font-size: 11px;
    color: var(--tone-sound);
  }

  .detail {
    padding: 4px 8px 12px 24px;
  }

  dl {
    display: grid;
    grid-template-columns: max-content 1fr;
    gap: 3px 16px;
    margin: 0 0 10px;
  }

  dt {
    color: var(--muted);
  }

  dd {
    margin: 0;
  }

  .files {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .file {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 2px 4px;
    border-radius: var(--radius);
  }

  .file.active {
    background: var(--accent-soft);
  }

  .placeholder {
    width: 28px;
  }

  section {
    margin-bottom: 16px;
  }

  h3 {
    margin: 12px 0 6px;
    font-size: 13px;
  }

  table {
    border-collapse: collapse;
    width: 100%;
    table-layout: fixed;
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
  }

  .code {
    margin: 0 8px 12px 24px;
    padding: 8px 10px;
    border: 1px solid var(--border);
    border-radius: var(--radius);
    background: var(--bg);
    font: 11px/1.5 var(--mono);
    white-space: pre-wrap;
    word-break: break-all;
  }

  .message {
    padding: 24px;
    text-align: center;
  }

  .error {
    color: var(--accent);
  }
</style>
