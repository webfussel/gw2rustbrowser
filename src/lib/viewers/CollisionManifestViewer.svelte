<script lang="ts">
  import { api, type CollisionEntry, type FileRef } from "../api";
  import { errorMessage } from "../format";
  import { showFile } from "../state.svelte";

  let { mft }: { mft: number } = $props();

  const ROW = 30;

  let entries = $state.raw<CollisionEntry[] | null>(null);
  let error = $state<string | null>(null);
  let filter = $state("");
  let scrollTop = $state(0);
  let viewport = $state(500);

  $effect(() => {
    const id = mft;
    entries = null;
    error = null;
    let stale = false;
    api
      .collisionManifest(id)
      .then((list) => !stale && (entries = list))
      .catch((e) => !stale && (error = errorMessage(e)));
    return () => (stale = true);
  });

  const shown = $derived.by(() => {
    const needle = filter.trim().toLowerCase();
    if (!entries || !needle) return entries ?? [];
    return entries.filter(
      (e) =>
        e.name.toLowerCase().includes(needle) ||
        String(e.model?.id ?? "").startsWith(needle) ||
        String(e.collision?.id ?? "").startsWith(needle),
    );
  });
  const first = $derived(Math.max(0, Math.floor(scrollTop / ROW) - 10));
  const visible = $derived(shown.slice(first, first + Math.ceil(viewport / ROW) + 20));

  /** Folder and file name of a `\`-separated asset path. */
  function split(path: string): [string, string] {
    const at = path.lastIndexOf("\\");
    return at < 0 ? ["", path] : [path.slice(0, at + 1), path.slice(at + 1)];
  }
</script>

{#snippet link(file: FileRef | null)}
  {#if file}
    <button class="link mono" title={file.kind ?? "not in archive"} onclick={() => showFile(file.id)}>{file.id}</button>
  {:else}
    <span class="muted">–</span>
  {/if}
{/snippet}

{#if error}
  <div class="message error">{error}</div>
{:else if !entries}
  <div class="message muted">Loading…</div>
{:else}
  <div class="bar">
    <input type="search" placeholder="Filter by asset path or file ID" bind:value={filter} />
    <span class="muted">
      {shown.length.toLocaleString()} of {entries.length.toLocaleString()} models with collision · original asset paths from
      development
    </span>
  </div>
  <div class="row head"><span>Asset</span><span>Model</span><span>Type</span><span>Collision</span></div>
  <div class="list" bind:clientHeight={viewport} onscroll={(e) => (scrollTop = e.currentTarget.scrollTop)}>
    <div class="spacer" style:height="{shown.length * ROW}px">
      {#each visible as e, i (first + i)}
        {@const [folder, name] = split(e.name)}
        <div class="row" style:top="{(first + i) * ROW}px">
          <span class="path" title={e.name}>{name} <span class="muted small">{folder}</span></span>
          <span>{@render link(e.model)}</span>
          <span class="muted small">{e.model?.kind ?? ""}</span>
          <span>{@render link(e.collision)}</span>
        </div>
      {/each}
    </div>
  </div>
{/if}

<style>
  .bar {
    display: flex;
    align-items: center;
    gap: 14px;
    min-height: 44px;
    padding: 4px 12px;
    background: var(--panel);
    border-bottom: 1px solid var(--border);
    flex: none;
  }

  .bar input {
    width: 320px;
  }

  .list {
    flex: 1;
    overflow: auto;
  }

  .spacer {
    position: relative;
  }

  .row {
    display: grid;
    grid-template-columns: 1fr 90px 140px 90px;
    gap: 12px;
    align-items: center;
    height: 30px;
    padding: 0 12px;
    border-bottom: 1px solid color-mix(in srgb, var(--border) 50%, transparent);
  }

  .list .row {
    position: absolute;
    left: 0;
    right: 0;
  }

  .row.head {
    flex: none;
    background: var(--panel-2);
    font-size: 11px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    color: var(--muted);
  }

  .path {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .small {
    font-size: 11px;
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
