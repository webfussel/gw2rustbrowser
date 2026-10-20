<script lang="ts">
  import { api, type EulaText } from "../api";
  import { errorMessage } from "../format";

  let { mft }: { mft: number } = $props();

  let texts = $state.raw<EulaText[] | null>(null);
  let error = $state<string | null>(null);
  let selected = $state(0);

  $effect(() => {
    let stale = false;
    api
      .loadEula(mft)
      .then((t) => !stale && (texts = t))
      .catch((e) => !stale && (error = errorMessage(e)));
    return () => (stale = true);
  });
</script>

{#if error}
  <div class="message error">{error}</div>
{:else if !texts}
  <div class="message muted">Loading…</div>
{:else}
  <div class="bar">
    {#each texts as t, i}
      <button class="btn small ghost" class:active={selected === i} onclick={() => (selected = i)}>{t.language}</button>
    {/each}
  </div>
  <pre>{texts[selected]?.text ?? ""}</pre>
{/if}

<style>
  .bar {
    display: flex;
    gap: 4px;
    align-items: center;
    height: 40px;
    padding: 0 12px;
    background: var(--panel);
    border-bottom: 1px solid var(--border);
    flex: none;
  }

  pre {
    flex: 1;
    margin: 0;
    padding: 16px 20px;
    overflow: auto;
    white-space: pre-wrap;
    font: 13px/1.6 var(--sans);
    user-select: text;
    background: var(--panel);
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
