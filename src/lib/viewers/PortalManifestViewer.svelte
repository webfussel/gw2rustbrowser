<script lang="ts">
  import { api, type PortalMap, type PortalModel } from "../api";
  import { errorMessage } from "../format";
  import { showFile } from "../state.svelte";

  let { mft }: { mft: number } = $props();

  const ROW = 28;

  let maps = $state.raw<PortalMap[] | null>(null);
  let error = $state<string | null>(null);
  let selected = $state(0);
  let start = $state<number | null>(null);
  let models = $state.raw<PortalModel[] | null>(null);
  let modelsError = $state<string | null>(null);
  let kindFilter = $state("");
  let scrollTop = $state(0);
  let viewport = $state(300);

  $effect(() => {
    const id = mft;
    maps = null;
    error = null;
    selected = 0;
    start = null;
    let stale = false;
    api
      .portalManifest(id)
      .then((list) => !stale && (maps = list))
      .catch((e) => !stale && (error = errorMessage(e)));
    return () => (stale = true);
  });

  $effect(() => {
    const [m, s] = [selected, start];
    models = null;
    modelsError = null;
    kindFilter = "";
    scrollTop = 0;
    if (s === null) return;
    let stale = false;
    api
      .portalStartModels(mft, m, s)
      .then((list) => !stale && (models = list))
      .catch((e) => !stale && (modelsError = errorMessage(e)));
    return () => (stale = true);
  });

  const map = $derived(maps?.[selected] ?? null);
  const total = (m: PortalMap) => m.starts.reduce((sum, s) => sum + s.models, 0);
  const shownModels = $derived(models?.filter((m) => !kindFilter || (m.file?.kind ?? "missing") === kindFilter) ?? []);
  const first = $derived(Math.max(0, Math.floor(scrollTop / ROW) - 10));
  const visible = $derived(shownModels.slice(first, first + Math.ceil(viewport / ROW) + 20));

  const hex = (n: number) => `0x${n.toString(16).padStart(8, "0")}`;
  const position = (p: [number, number, number]) => p.map((v) => Math.round(v).toLocaleString()).join(", ");
  const kindLabel = (kind: string) => kind.replace(/^MODL /, "");
</script>

{#if error}
  <div class="message error">{error}</div>
{:else if !maps}
  <div class="message muted">Loading…</div>
{:else if maps.length === 0}
  <div class="message muted">This manifest lists no maps.</div>
{:else}
  <div class="layout">
    <aside>
      <div class="heading muted">{maps.length} maps</div>
      <div class="maps">
        {#each maps as m, i (i)}
          <button class="item" class:active={selected === i} onclick={() => ((selected = i), (start = null))}>
            <span class="mono guid">{m.guid}</span>
            <span class="muted small">
              {m.starts.length} starts · {total(m).toLocaleString()} models{m.redirectors.length
                ? ` · ${m.redirectors.length} exits`
                : ""}
            </span>
          </button>
        {/each}
      </div>
    </aside>

    {#if map}
      <div class="detail">
        <div class="top" class:full={start === null}>
          <h2 class="mono">{map.guid}</h2>
          <p class="muted small">
            Map GUID of the game content (not the GUID stored in the map files). Positions are world units of that map.
          </p>
          <h3>Start points</h3>
          <div class="table">
            <div class="trow head"><span>Token</span><span>Position</span><span>Radius</span><span>Models</span></div>
            {#each map.starts as s, i (i)}
              <button class="trow" class:active={start === i} onclick={() => (start = start === i ? null : i)}>
                <span class="mono">{hex(s.token)}</span>
                <span class="mono small">{position(s.position)}</span>
                <span class="mono small">{Math.round(s.radius)}</span>
                <span class="kinds">
                  <b>{s.models.toLocaleString()}</b>
                  {#each Object.entries(s.kinds).sort((a, b) => b[1] - a[1]) as [kind, n] (kind)}
                    <span class="chip" class:missing={kind === "missing"}>{n.toLocaleString()} {kindLabel(kind)}</span>
                  {/each}
                </span>
              </button>
            {/each}
          </div>
          {#if map.redirectors.length}
            <h3>Exits to other maps</h3>
            <div class="table">
              <div class="trow head"><span>Token</span><span>Position</span><span>Radius</span><span>Target map</span></div>
              {#each map.redirectors as r, i (i)}
                <div class="trow">
                  <span class="mono">{hex(r.token)}</span>
                  <span class="mono small">{position(r.position)}</span>
                  <span class="mono small">{Math.round(r.radius)}</span>
                  <span class="mono small">{r.guid}</span>
                </div>
              {/each}
            </div>
          {/if}
          {#if start === null}<p class="muted small">Select a start point to list the models loaded for it.</p>{/if}
        </div>

        {#if start !== null}
          <div class="models">
            <div class="bar">
              <strong>Models of start {start + 1}</strong>
              {#if models}
                <select bind:value={kindFilter}>
                  <option value="">All types ({models.length.toLocaleString()})</option>
                  {#each Object.entries(map.starts[start].kinds) as [kind, n] (kind)}
                    <option value={kind}>{kindLabel(kind)} ({n.toLocaleString()})</option>
                  {/each}
                </select>
              {/if}
            </div>
            {#if modelsError}
              <div class="message error">{modelsError}</div>
            {:else if !models}
              <div class="message muted">Loading models…</div>
            {:else}
              <div class="mrow head"><span>Model</span><span>Type</span><span>Flags</span><span>Permutation</span></div>
              <div class="list" bind:clientHeight={viewport} onscroll={(e) => (scrollTop = e.currentTarget.scrollTop)}>
                <div class="spacer" style:height="{shownModels.length * ROW}px">
                  {#each visible as m, i (first + i)}
                    <div class="mrow" style:top="{(first + i) * ROW}px">
                      <span>
                        {#if m.file}
                          <button class="link mono" onclick={() => showFile(m.file!.id)}>{m.file.id}</button>
                        {:else}
                          <span class="muted">none</span>
                        {/if}
                      </span>
                      <span class="muted small">{m.file?.kind ?? ""}{m.file && m.file.mft < 0 ? "not in archive" : ""}</span>
                      <span class="mono small">{m.flags ? `0x${m.flags.toString(16)}` : ""}</span>
                      <span class="mono small muted">{m.permutation === "0x0" ? "" : m.permutation}</span>
                    </div>
                  {/each}
                </div>
              </div>
            {/if}
          </div>
        {/if}
      </div>
    {/if}
  </div>
{/if}

<style>
  .layout {
    flex: 1;
    display: flex;
    min-height: 0;
  }

  aside {
    width: 330px;
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

  .maps {
    flex: 1;
    overflow: auto;
  }

  .item {
    width: 100%;
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 2px;
    padding: 6px 12px;
    border: 0;
    background: none;
    color: inherit;
    font: inherit;
    cursor: pointer;
    text-align: left;
  }

  .item:hover {
    background: var(--row-hover);
  }

  .item.active {
    background: var(--accent-soft);
    box-shadow: inset 2px 0 var(--accent);
  }

  .guid {
    font-size: 11px;
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
    max-height: 50%;
    overflow: auto;
    padding: 14px 18px 8px;
  }

  .top.full {
    flex: 1;
    max-height: none;
  }

  h2 {
    margin: 0;
    font-size: 15px;
  }

  h3 {
    margin: 14px 0 6px;
    font-size: 13px;
  }

  p {
    margin: 4px 0;
  }

  .trow {
    width: 100%;
    display: grid;
    grid-template-columns: 110px 220px 70px 1fr;
    gap: 10px;
    align-items: center;
    padding: 4px 6px;
    border: 0;
    border-bottom: 1px solid var(--border);
    background: none;
    color: inherit;
    font: inherit;
    text-align: left;
  }

  button.trow {
    cursor: pointer;
  }

  button.trow:hover {
    background: var(--row-hover);
  }

  .trow.active {
    background: var(--accent-soft);
  }

  .trow.head,
  .mrow.head {
    color: var(--muted);
    font-size: 11px;
  }

  .kinds {
    display: flex;
    flex-wrap: wrap;
    gap: 4px 8px;
    align-items: center;
  }

  .chip {
    padding: 0 7px;
    border: 1px solid var(--border);
    border-radius: 999px;
    font-size: 11px;
    color: var(--muted);
  }

  .chip.missing {
    color: var(--accent);
  }

  .models {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
    border-top: 1px solid var(--border);
  }

  .bar {
    display: flex;
    align-items: center;
    gap: 12px;
    min-height: 38px;
    padding: 4px 12px;
    background: var(--panel);
    flex: none;
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

  .list {
    flex: 1;
    overflow: auto;
  }

  .spacer {
    position: relative;
  }

  .mrow {
    display: grid;
    grid-template-columns: 100px 140px 70px 1fr;
    gap: 10px;
    align-items: center;
    height: 28px;
    padding: 0 12px;
    border-bottom: 1px solid color-mix(in srgb, var(--border) 50%, transparent);
  }

  .list .mrow {
    position: absolute;
    left: 0;
    right: 0;
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
