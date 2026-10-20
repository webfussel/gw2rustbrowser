<script lang="ts">
  import { api } from "../api";
  import { errorMessage } from "../format";

  let { mft }: { mft: number } = $props();

  const LIMIT = 4 * 1024 * 1024;
  let text = $state<string | null>(null);
  let error = $state<string | null>(null);
  let truncated = $state(false);
  let wrap = $state(true);

  $effect(() => {
    let stale = false;
    api
      .loadText(mft)
      .then((full) => {
        if (stale) return;
        truncated = full.length > LIMIT;
        text = truncated ? full.slice(0, LIMIT) : full;
      })
      .catch((e) => !stale && (error = errorMessage(e)));
    return () => (stale = true);
  });
</script>

<div class="bar">
  <label><input type="checkbox" bind:checked={wrap} /> Wrap lines</label>
  {#if truncated}<span class="muted">Showing the first 4 million characters</span>{/if}
</div>
{#if error}
  <div class="message error">{error}</div>
{:else if text === null}
  <div class="message muted">Loading…</div>
{:else}
  <pre class="mono" class:wrap>{text}</pre>
{/if}

<style>
  .bar {
    display: flex;
    gap: 16px;
    align-items: center;
    height: 34px;
    padding: 0 12px;
    background: var(--panel);
    border-bottom: 1px solid var(--border);
    flex: none;
  }

  pre {
    flex: 1;
    margin: 0;
    padding: 12px 16px;
    overflow: auto;
    user-select: text;
    background: var(--panel);
    tab-size: 4;
  }

  pre.wrap {
    white-space: pre-wrap;
    word-break: break-word;
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
