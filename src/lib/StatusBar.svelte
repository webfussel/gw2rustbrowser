<script lang="ts">
  import { browser } from "./state.svelte";

  $effect(() => {
    const notice = browser.notice;
    if (!notice || notice.kind === "error") return;
    const timer = setTimeout(() => {
      if (browser.notice === notice) browser.notice = null;
    }, 8000);
    return () => clearTimeout(timer);
  });
</script>

<footer>
  {#if browser.summary}
    <span>{browser.summary.files.toLocaleString()} files</span>
    <span class="sep">·</span>
    <span>{browser.summary.categories.toLocaleString()} categories</span>
  {:else}
    <span class="muted">No archive open</span>
  {/if}
  {#if browser.notice}
    <button class="notice {browser.notice.kind}" onclick={() => (browser.notice = null)} title="Dismiss">
      {browser.notice.text}
    </button>
  {/if}
</footer>

<style>
  footer {
    display: flex;
    align-items: center;
    gap: 8px;
    height: 26px;
    padding: 0 12px;
    background: var(--panel);
    border-top: 1px solid var(--border);
    font-size: 12px;
    color: var(--muted);
    flex: none;
  }

  .sep {
    color: var(--faint);
  }

  .notice {
    margin-left: auto;
    border: 0;
    background: none;
    padding: 0;
    color: var(--text);
    font-size: 12px;
    max-width: 70%;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    cursor: pointer;
  }

  .notice.error {
    color: var(--accent);
  }
</style>
