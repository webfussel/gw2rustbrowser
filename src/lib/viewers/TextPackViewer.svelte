<script lang="ts">
  import { api, type TextPackInfo, type TextRow, type VariantRow } from "../api";
  import { errorMessage } from "../format";
  import { notify } from "../state.svelte";

  /** `txtm` = all texts, `txtV` = variants, `txtv` = voice lines. */
  let { type }: { type: string } = $props();

  type Row = TextRow | VariantRow;
  const PAGE = 200;

  const mode = $derived(type === "txtV" ? "variants" : type === "txtv" ? "voices" : "texts");
  const rowHeight = $derived(mode === "variants" ? 52 : 30);

  let info = $state<TextPackInfo | null>(null);
  let error = $state<string | null>(null);
  let language = $state(0);
  let pages = $state.raw(new Map<number, Row[]>());
  let loading = new Set<number>();
  let generation = 0;
  let scrollTop = $state(0);
  let height = $state(400);
  let viewport: HTMLDivElement | undefined = $state();
  let query = $state("");
  let results = $state.raw<TextRow[] | null>(null);
  let searching = $state(false);
  let goTo = $state("");
  let indexing = $state(false);
  let audioUrl = $state<string | null>(null);
  // Almost all voice lines have encrypted text, but many can be played.
  let playableOnly = $state(true);
  let voiceTotal = $state<number | null>(null);
  let playing = $state<number | null>(null);

  $effect(() => {
    api
      .textpackInfo()
      .then((i) => {
        info = i;
        language = i.languages[0]?.index ?? 0;
        if (!i.voiceIndexReady && i.voiceCount > 0) buildVoiceIndex();
      })
      .catch((e) => (error = errorMessage(e)));
  });

  $effect(() => () => {
    if (audioUrl) URL.revokeObjectURL(audioUrl);
  });

  async function buildVoiceIndex() {
    indexing = true;
    try {
      await api.buildVoiceIndex();
      reload();
    } catch (e) {
      notify(errorMessage(e), "error");
    } finally {
      indexing = false;
    }
  }

  const total = $derived(
    results
      ? results.length
      : !info
        ? 0
        : mode === "texts"
          ? info.textCount
          : mode === "variants"
            ? info.variantCount
            : (voiceTotal ?? info.voiceCount),
  );
  const first = $derived(Math.max(0, Math.floor(scrollTop / rowHeight) - 5));
  const last = $derived(Math.min(total, Math.ceil((scrollTop + height) / rowHeight) + 5));

  function reload() {
    generation++;
    loading = new Set();
    pages = new Map();
    voiceTotal = null;
    if (results && query.trim()) search();
  }

  async function fetchPage(page: number) {
    if (loading.has(page) || pages.has(page) || !info) return;
    loading.add(page);
    const gen = generation;
    try {
      const start = page * PAGE;
      let rows: Row[];
      if (mode === "voices") {
        const page = await api.voiceRows(language, start, PAGE, playableOnly);
        if (gen !== generation) return;
        voiceTotal = page.total;
        rows = page.rows;
      } else {
        rows = mode === "texts" ? await api.textRows(language, start, PAGE) : await api.variantRows(language, start, PAGE);
      }
      if (gen !== generation) return;
      pages = new Map(pages).set(page, rows);
    } catch (e) {
      error = errorMessage(e);
    } finally {
      loading.delete(page);
    }
  }

  // Load the pages of the visible rows.
  $effect(() => {
    if (results || !info) return;
    void language;
    for (let p = Math.floor(first / PAGE); p <= Math.floor(Math.max(first, last - 1) / PAGE); p++) fetchPage(p);
  });

  function rowAt(i: number): Row | undefined {
    if (results) return results[i];
    return pages.get(Math.floor(i / PAGE))?.[i % PAGE];
  }

  async function search() {
    const q = query.trim();
    if (!q) {
      results = null;
      return;
    }
    searching = true;
    try {
      results = await api.searchTexts(language, q);
      if (viewport) viewport.scrollTop = 0;
    } catch (e) {
      notify(errorMessage(e), "error");
    } finally {
      searching = false;
    }
  }

  function clearSearch() {
    query = "";
    results = null;
  }

  function jump() {
    const id = Number(goTo);
    if (!Number.isFinite(id) || !viewport) return;
    clearSearch();
    viewport.scrollTop = Math.max(0, id) * rowHeight;
  }

  async function play(row: TextRow) {
    if (row.voice === null) return;
    try {
      const sound = await api.playVoice(row.voice);
      if (audioUrl) URL.revokeObjectURL(audioUrl);
      audioUrl = URL.createObjectURL(new Blob([sound.data], { type: sound.mime }));
      playing = row.id;
    } catch (e) {
      notify(errorMessage(e), "error");
    }
  }

  function setLanguage(value: number) {
    language = value;
    reload();
  }
</script>

{#snippet text(row: TextRow)}
  {#if row.state === "plain"}
    {#if row.text}<span class="text" title={row.text}>{row.text}</span>{:else}<span class="faint">(empty)</span>{/if}
  {:else}
    <span class="faint">{row.state === "encrypted" ? "encrypted" : "not in this language"}</span>
  {/if}
{/snippet}

{#snippet voice(row: TextRow)}
  {#if row.voice !== null}
    {#if row.playable}
      <button class="btn small ghost play" class:active={playing === row.id} onclick={() => play(row)} title="Play voice line {row.voice}"
        >▶</button
      >
    {:else if row.playable === false}
      <span class="faint" title="Voice line {row.voice} is encrypted">🔒</span>
    {:else}
      <span class="faint" title="Indexing voice lines…">…</span>
    {/if}
  {/if}
{/snippet}

{#if error}
  <div class="message error">{error}</div>
{:else if !info}
  <div class="message muted">Loading…</div>
{:else}
  <div class="bar">
    <select value={language} onchange={(e) => setLanguage(Number(e.currentTarget.value))} title="Language">
      {#each info.languages as l (l.index)}
        <option value={l.index}>{l.name}</option>
      {/each}
    </select>
    {#if mode !== "variants"}
      <input
        type="search"
        placeholder="Search all texts…"
        bind:value={query}
        onkeydown={(e) => e.key === "Enter" && search()}
        oninput={() => !query && clearSearch()}
      />
      {#if searching}<span class="muted">Searching…</span>{/if}
    {/if}
    {#if mode === "voices"}
      <label title="Most voice lines have encrypted audio (keys come from the game server)">
        <input type="checkbox" bind:checked={playableOnly} onchange={reload} disabled={indexing} /> Only playable
      </label>
    {/if}
    {#if mode === "texts"}
      <input class="goto" type="text" placeholder="Go to text id" bind:value={goTo} onkeydown={(e) => e.key === "Enter" && jump()} />
    {/if}
    <span class="muted info">
      {#if results}
        {results.length.toLocaleString()}{results.length >= 2000 ? "+" : ""} matches
      {:else if mode === "texts"}
        {info.textCount.toLocaleString()} text ids · {info.voiceCount.toLocaleString()} voiced
      {:else if mode === "variants"}
        {info.variantCount.toLocaleString()} texts with variants
      {:else}
        {(voiceTotal ?? info.voiceCount).toLocaleString()} voice lines{playableOnly && !indexing ? " with playable audio" : ""} · their
        texts are almost all encrypted
      {/if}
      {#if indexing}· indexing voice lines…{/if}
    </span>
    {#if audioUrl}
      <audio controls autoplay src={audioUrl}></audio>
    {/if}
  </div>

  <div class="viewport" bind:this={viewport} bind:clientHeight={height} onscroll={(e) => (scrollTop = e.currentTarget.scrollTop)}>
    <div class="spacer" style:height="{total * rowHeight}px">
      {#each { length: last - first } as _, k (first + k)}
        {@const i = first + k}
        {@const row = rowAt(i)}
        <div class="row" class:tall={mode === "variants"} style:transform="translateY({i * rowHeight}px)" style:height="{rowHeight}px">
          {#if !row}
            <span class="mono faint">{mode === "texts" && !results ? i : ""}</span><span class="faint">…</span>
          {:else}
            <span class="mono id">{row.id}</span>
            <div class="content">
              <div class="line">{@render text(row)}{@render voice(row)}</div>
              {#if "variants" in row}
                <div class="variants">
                  {#each row.variants as v (v.id)}
                    <span class="variant"><span class="mono faint">{v.id}</span> {@render text(v)}</span>
                  {/each}
                </div>
              {/if}
            </div>
          {/if}
        </div>
      {/each}
    </div>
  </div>
{/if}

<style>
  .bar {
    display: flex;
    align-items: center;
    gap: 10px;
    min-height: 44px;
    padding: 4px 12px;
    background: var(--panel);
    border-bottom: 1px solid var(--border);
    flex: none;
    flex-wrap: wrap;
  }

  select {
    height: 28px;
    padding: 0 6px;
    border: 1px solid var(--border);
    border-radius: var(--radius);
    background: var(--panel-2);
    color: var(--text);
    font: inherit;
  }

  input[type="search"] {
    width: 240px;
  }

  .goto {
    width: 130px;
  }

  .info {
    font-size: 12px;
  }

  audio {
    margin-left: auto;
    height: 32px;
  }

  .viewport {
    flex: 1;
    overflow: auto;
    user-select: text;
  }

  .spacer {
    position: relative;
  }

  .row {
    position: absolute;
    left: 0;
    right: 0;
    display: grid;
    grid-template-columns: 84px 1fr;
    gap: 12px;
    align-items: center;
    padding: 0 12px;
    border-bottom: 1px solid color-mix(in srgb, var(--border) 50%, transparent);
  }

  .row.tall {
    align-items: start;
    padding-top: 5px;
  }

  .id {
    color: var(--muted);
  }

  .content {
    min-width: 0;
  }

  .line {
    display: flex;
    align-items: center;
    gap: 8px;
    min-width: 0;
  }

  .text {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .faint {
    color: var(--faint);
    font-style: italic;
  }

  .variants {
    display: flex;
    gap: 14px;
    overflow: hidden;
    white-space: nowrap;
    font-size: 12px;
    color: var(--muted);
  }

  .variant {
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .play {
    height: 20px;
    padding: 0 6px;
    font-size: 10px;
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
