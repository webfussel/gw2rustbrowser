<script lang="ts">
  import { save } from "@tauri-apps/plugin-dialog";
  import { api, type BankEntry } from "../api";
  import { errorMessage, formatBytes } from "../format";
  import { notify } from "../state.svelte";

  let { mft, name }: { mft: number; name: string } = $props();

  let entries = $state.raw<BankEntry[] | null>(null);
  let error = $state<string | null>(null);
  let playableOnly = $state(true);
  let playing = $state<number | null>(null);
  let url = $state<string | null>(null);

  $effect(() => {
    let stale = false;
    api
      .loadBank(mft)
      .then((list) => !stale && (entries = list))
      .catch((e) => !stale && (error = errorMessage(e)));
    return () => {
      stale = true;
    };
  });

  $effect(() => () => {
    if (url) URL.revokeObjectURL(url);
  });

  const shown = $derived(entries?.filter((e) => !playableOnly || e.codec) ?? []);
  const playableCount = $derived(entries?.filter((e) => e.codec).length ?? 0);

  async function play(entry: BankEntry) {
    try {
      const sound = await api.loadBankSound(mft, entry.index);
      if (url) URL.revokeObjectURL(url);
      url = URL.createObjectURL(new Blob([sound.data], { type: sound.mime }));
      playing = entry.index;
    } catch (e) {
      notify(errorMessage(e), "error");
    }
  }

  async function exportEntry(entry: BankEntry) {
    const defaultPath = `${name}_${String(entry.index).padStart(3, "0")}.${entry.codec}`;
    const path = await save({ defaultPath, filters: [{ name: entry.codec!.toUpperCase(), extensions: [entry.codec!] }] });
    if (!path) return;
    try {
      await api.exportBankSound(mft, entry.index, path);
      notify(`Exported to ${path}`);
    } catch (e) {
      notify(errorMessage(e), "error");
    }
  }
</script>

<div class="bar">
  {#if entries}
    <span>{playableCount} of {entries.length} sounds playable</span>
    <label class="muted"><input type="checkbox" bind:checked={playableOnly} /> Hide encrypted</label>
  {/if}
  {#if url}
    <audio controls autoplay src={url}></audio>
  {/if}
</div>
{#if error}
  <div class="message error">{error}</div>
{:else if !entries}
  <div class="message muted">Loading…</div>
{:else}
  <div class="table">
    <div class="row head">
      <span>#</span><span>Voice ID</span><span>Format</span><span>Length</span><span class="right">Size</span><span></span>
    </div>
    {#each shown as entry (entry.index)}
      <div class="row" class:active={playing === entry.index} class:locked={!entry.codec}>
        <span class="mono">{entry.index}</span>
        <span class="mono muted">{entry.voiceId}</span>
        <span>{entry.codec ? entry.codec.toUpperCase() : "encrypted"}</span>
        <span class="mono muted">{entry.length ? `${entry.length.toFixed(2)} s` : ""}</span>
        <span class="mono muted right">{entry.codec ? formatBytes(entry.size) : ""}</span>
        <span class="actions">
          {#if entry.codec}
            <button class="btn small ghost" onclick={() => play(entry)} aria-label="Play">▶</button>
            <button class="btn small ghost" onclick={() => exportEntry(entry)}>Export</button>
          {/if}
        </span>
      </div>
    {/each}
    {#if shown.length === 0}
      <div class="message muted">All sounds in this bank are encrypted.</div>
    {/if}
  </div>
{/if}

<style>
  .bar {
    display: flex;
    align-items: center;
    gap: 16px;
    min-height: 44px;
    padding: 4px 12px;
    background: var(--panel);
    border-bottom: 1px solid var(--border);
    flex: none;
  }

  audio {
    margin-left: auto;
    height: 32px;
  }

  .table {
    flex: 1;
    overflow: auto;
  }

  .row {
    display: grid;
    grid-template-columns: 50px 1fr 90px 80px 90px 130px;
    gap: 8px;
    align-items: center;
    height: 30px;
    padding: 0 12px;
    border-bottom: 1px solid color-mix(in srgb, var(--border) 50%, transparent);
  }

  .row.head {
    position: sticky;
    top: 0;
    background: var(--panel-2);
    font-size: 11px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    color: var(--muted);
  }

  .row.active {
    background: var(--accent-soft);
  }

  .row.locked {
    color: var(--faint);
  }

  .right {
    text-align: right;
  }

  .actions {
    display: flex;
    justify-content: flex-end;
    gap: 2px;
  }

  .message {
    padding: 24px;
    text-align: center;
  }

  .error {
    color: var(--accent);
  }
</style>
