<script lang="ts">
  import { api } from "../api";
  import { errorMessage } from "../format";

  let { mft }: { mft: number } = $props();

  let maps = $state.raw<[number, number][] | null>(null);
  let error = $state<string | null>(null);
  let type = $state<number | null>(null);
  let filter = $state("");

  $effect(() => {
    const id = mft;
    maps = null;
    let stale = false;
    api
      .mapMetadata(id)
      .then((list) => !stale && (maps = list))
      .catch((e) => !stale && (error = errorMessage(e)));
    return () => (stale = true);
  });

  const types = $derived.by(() => {
    const counts = new Map<number, number>();
    for (const [, t] of maps ?? []) counts.set(t, (counts.get(t) ?? 0) + 1);
    return [...counts.entries()].sort((a, b) => a[0] - b[0]);
  });
  const shown = $derived(
    (maps ?? []).filter(([id, t]) => (type === null || t === type) && (!filter.trim() || String(id).startsWith(filter.trim()))),
  );
</script>

{#if error}
  <div class="message error">{error}</div>
{:else if !maps}
  <div class="message muted">Loading…</div>
{:else}
  <div class="bar">
    <input type="search" placeholder="Map id" bind:value={filter} />
    <span class="types">
      <button class="btn small" class:ghost={type !== null} onclick={() => (type = null)}>All ({maps.length})</button>
      {#each types as [t, n] (t)}
        <button class="btn small" class:ghost={type !== t} onclick={() => (type = t)}>Type {t} ({n})</button>
      {/each}
    </span>
  </div>
  <p class="note muted">
    Map ids are the ones the game and its web API use (e.g. 15 is Queensdale). What the type numbers mean is not known;
    the open-world maps 15–25 all have type 5.
  </p>
  <div class="grid">
    {#each shown as [id, t] (id)}
      <span class="cell"><span class="mono">{id}</span><span class="muted small">type {t}</span></span>
    {/each}
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

  .bar input {
    width: 120px;
  }

  .types {
    display: flex;
    flex-wrap: wrap;
    gap: 4px;
  }

  .note {
    margin: 0;
    padding: 6px 12px;
    font-size: 11px;
    flex: none;
  }

  .grid {
    flex: 1;
    overflow: auto;
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(110px, 1fr));
    align-content: start;
    gap: 4px;
    padding: 8px 12px 16px;
  }

  .cell {
    display: flex;
    justify-content: space-between;
    align-items: baseline;
    padding: 3px 8px;
    border: 1px solid var(--border);
    border-radius: var(--radius);
  }

  .small {
    font-size: 11px;
  }

  .message {
    padding: 24px;
    text-align: center;
  }

  .error {
    color: var(--accent);
  }
</style>
