<script lang="ts">
  import { api, type Emote } from "../api";
  import { errorMessage } from "../format";
  import { showFile } from "../state.svelte";
  import ModelSummaryCard from "./ModelSummaryCard.svelte";

  let { mft }: { mft: number } = $props();

  // Model types without meshes get a summary instead of the 3D view.
  const meshless = ["MODL skeleton", "MODL animset", "MODL materials", "MODL empty"];
  const modelViewer = () => import("./ModelViewer.svelte");

  let emotes = $state.raw<Emote[] | null>(null);
  let error = $state<string | null>(null);
  let filter = $state("");
  let selected = $state(0);
  let timing = $state<number | null>(null);

  $effect(() => {
    const id = mft;
    emotes = null;
    error = null;
    let stale = false;
    api
      .emoteInfo(id)
      .then((list) => !stale && ((emotes = list), (selected = 0), (timing = null)))
      .catch((e) => !stale && (error = errorMessage(e)));
    return () => (stale = true);
  });

  const shown = $derived.by(() => {
    const needle = filter.trim().toLowerCase();
    return (emotes ?? []).map((e, i) => ({ e, i })).filter(({ e }) => !needle || e.name.includes(needle));
  });
  const emote = $derived(emotes?.[selected] ?? null);
  const longest = $derived(Math.max(1, ...(emote?.timings.map((t) => t.intro + t.loopLength + t.outro) ?? [1])));
  const current = $derived(emote && timing !== null ? (emote.timings[timing] ?? null) : null);

  function select(i: number) {
    selected = i;
    timing = null;
  }

  const seconds = (ms: number) => `${(ms / 1000).toFixed(2)} s`;
</script>

{#if error}
  <div class="message error">{error}</div>
{:else if !emotes}
  <div class="message muted">Loading emotes…</div>
{:else}
  <div class="layout">
    <aside>
      <input type="search" placeholder="Filter {emotes.length} emotes…" bind:value={filter} />
      <div class="list">
        {#each shown as { e, i } (i)}
          <button class="row" class:active={selected === i} onclick={() => select(i)}>
            <span class="mono">{e.name}</span>
            <span class="muted small">{e.timings.length}</span>
          </button>
        {/each}
      </div>
    </aside>

    <div class="detail">
      {#if emote}
        <div class="top">
          <header>
            <h2 class="mono">{emote.name}</h2>
            <span class="muted">
              {emote.timings.length} {emote.timings.length === 1 ? "variant" : "variants"} (one per body type, mount or prop)
            </span>
          </header>
          <div class="legend muted small">
            <span><i class="intro"></i>intro</span><span><i class="loop"></i>loop</span><span><i class="outro"></i>outro</span>
          </div>
          <div class="timings">
            <div class="trow head">
              <span>Animation file</span><span>Type</span><span>Timeline</span><span>Blend in / out</span><span>Start</span>
            </div>
            {#each emote.timings as t, i (i)}
              <div
                class="trow"
                class:active={timing === i}
                role="button"
                tabindex="0"
                onclick={() => (timing = timing === i ? null : i)}
                onkeydown={(ev) => ev.key === "Enter" && (timing = timing === i ? null : i)}
              >
                <span>
                  {#if t.fileId}
                    <button
                      class="link mono"
                      title="Show this file in the browser"
                      onclick={(ev) => (ev.stopPropagation(), showFile(t.fileId!))}>{t.fileId}</button
                    >
                  {:else}
                    <span class="muted">none</span>
                  {/if}
                </span>
                <span class="muted small">{t.fileType ?? (t.fileId ? "not in archive" : "")}</span>
                <span class="timeline" title="intro {seconds(t.intro)} · loop {seconds(t.loopLength)} · outro {seconds(t.outro)}">
                  {#if t.intro}<i class="intro" style:width="{(t.intro / longest) * 100}%"></i>{/if}
                  {#if t.loopLength}<i class="loop" style:width="{(t.loopLength / longest) * 100}%"></i>{/if}
                  {#if t.outro}<i class="outro" style:width="{(t.outro / longest) * 100}%"></i>{/if}
                  <span class="mono small total">{seconds(t.intro + t.loopLength + t.outro)}</span>
                </span>
                <span class="mono small">{t.blendIn.toFixed(2)} / {t.blendOut.toFixed(2)} s</span>
                <span class="mono small muted">{t.startOffset ? `${t.startOffset} ms` : ""}</span>
              </div>
            {/each}
          </div>
          <p class="muted small hint">Click a variant to look at its animation file below.</p>
        </div>

        {#if current && current.mft >= 0 && current.fileType}
          <div class="model">
            {#key current.mft}
              {#if meshless.includes(current.fileType)}
                <ModelSummaryCard mft={current.mft} type={current.fileType} />
              {:else}
                {#await modelViewer() then { default: ModelViewer }}
                  <ModelViewer mft={current.mft} />
                {/await}
              {/if}
            {/key}
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
    width: 220px;
    flex: none;
    display: flex;
    flex-direction: column;
    background: var(--panel);
    border-right: 1px solid var(--border);
  }

  aside input {
    margin: 10px;
  }

  .list {
    flex: 1;
    overflow: auto;
  }

  .row {
    width: 100%;
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 5px 12px;
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

  .top {
    flex: none;
    max-height: 55%;
    overflow: auto;
    padding: 14px 18px 8px;
  }

  header {
    display: flex;
    align-items: baseline;
    gap: 12px;
    margin-bottom: 8px;
    flex-wrap: wrap;
  }

  h2 {
    margin: 0;
    font-size: 18px;
  }

  .legend {
    display: flex;
    gap: 14px;
    margin-bottom: 6px;
  }

  .legend span {
    display: flex;
    align-items: center;
    gap: 5px;
  }

  .legend i {
    width: 12px;
    height: 8px;
    border-radius: 2px;
  }

  .trow {
    display: grid;
    grid-template-columns: 90px 120px 1fr 110px 70px;
    gap: 10px;
    align-items: center;
    padding: 4px 6px;
    border-bottom: 1px solid var(--border);
    cursor: pointer;
  }

  .trow.head {
    cursor: default;
    color: var(--muted);
    font-size: 11px;
  }

  .trow:not(.head):hover {
    background: var(--row-hover);
  }

  .trow.active {
    background: var(--accent-soft);
  }

  .timeline {
    position: relative;
    display: flex;
    height: 14px;
    border-radius: 3px;
    background: var(--panel-2);
    overflow: hidden;
  }

  .total {
    position: absolute;
    right: 4px;
    top: 0;
    line-height: 14px;
    color: var(--text);
  }

  i.intro {
    background: var(--tone-text);
  }

  i.loop {
    background: var(--tone-animation);
  }

  i.outro {
    background: var(--tone-map);
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

  .hint {
    margin: 8px 0 0;
  }

  .model {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
    border-top: 1px solid var(--border);
    overflow: auto;
  }

  .message {
    padding: 24px;
    text-align: center;
  }

  .error {
    color: var(--accent);
  }
</style>
