<script lang="ts">
  import { api, type StringsData } from "../api";
  import { errorMessage } from "../format";

  let { mft }: { mft: number } = $props();

  const ROW = 28;
  let data = $state.raw<StringsData | null>(null);
  let error = $state<string | null>(null);
  let filter = $state("");
  let showEncrypted = $state(false);
  let scrollTop = $state(0);
  let height = $state(400);

  $effect(() => {
    let stale = false;
    api
      .loadStrings(mft)
      .then((d) => !stale && (data = d))
      .catch((e) => !stale && (error = errorMessage(e)));
    return () => (stale = true);
  });

  const rows = $derived.by(() => {
    if (!data) return [];
    const needle = filter.trim().toLowerCase();
    return data.entries.filter(([index, text]) => {
      if (text === null) return showEncrypted && !needle;
      return !needle || text.toLowerCase().includes(needle) || String(index) === needle;
    });
  });
  const encrypted = $derived(data?.entries.filter(([, t]) => t === null).length ?? 0);
  const first = $derived(Math.max(0, Math.floor(scrollTop / ROW) - 6));
  const last = $derived(Math.min(rows.length, Math.ceil((scrollTop + height) / ROW) + 6));
</script>

<div class="bar">
  <input type="search" placeholder="Filter strings…" bind:value={filter} />
  {#if data}
    <span class="muted">{data.language} · {rows.length.toLocaleString()} shown · {encrypted.toLocaleString()} encrypted</span>
    <label class="muted"><input type="checkbox" bind:checked={showEncrypted} /> Show encrypted</label>
  {/if}
</div>
{#if error}
  <div class="message error">{error}</div>
{:else if !data}
  <div class="message muted">Loading…</div>
{:else}
  <div class="viewport" bind:clientHeight={height} onscroll={(e) => (scrollTop = e.currentTarget.scrollTop)}>
    <div class="spacer" style:height="{rows.length * ROW}px">
      {#each rows.slice(first, last) as [index, text], i (index)}
        <div class="row" style:transform="translateY({(first + i) * ROW}px)">
          <span class="mono muted">{index}</span>
          {#if text === null}
            <span class="locked">encrypted</span>
          {:else}
            <span class="text" title={text}>{text}</span>
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
    gap: 16px;
    height: 44px;
    padding: 0 12px;
    background: var(--panel);
    border-bottom: 1px solid var(--border);
    flex: none;
  }

  .bar input[type="search"] {
    width: 260px;
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
    grid-template-columns: 64px 1fr;
    gap: 12px;
    align-items: center;
    height: 28px;
    padding: 0 12px;
    border-bottom: 1px solid color-mix(in srgb, var(--border) 50%, transparent);
  }

  .text {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .locked {
    color: var(--faint);
    font-style: italic;
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
