<script lang="ts">
  import { browser, clearSearch, openArchive, search } from "./state.svelte";

  let query = $state("");
  let input: HTMLInputElement | undefined = $state();

  function onKey(event: KeyboardEvent) {
    if (event.key === "Enter") search(query);
    if (event.key === "Escape") {
      query = "";
      clearSearch();
    }
  }

  function globalKey(event: KeyboardEvent) {
    if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === "f" && browser.summary) {
      event.preventDefault();
      input?.focus();
      input?.select();
    }
    if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === "o") {
      event.preventDefault();
      openArchive();
    }
  }
</script>

<svelte:window onkeydown={globalKey} />

<header>
  <div class="brand">
    <svg viewBox="0 0 24 24" width="18" height="18" aria-hidden="true">
      <path d="M12 2 21 7v10l-9 5-9-5V7z" fill="var(--accent)" />
      <path d="M12 2 21 7l-9 5-9-5z" fill="#fff" fill-opacity=".25" />
    </svg>
    <span>GW2 Browser</span>
  </div>
  <button class="btn" onclick={() => openArchive()} title="Open a Gw2.dat (Ctrl+O)">Open…</button>
  {#if browser.summary}
    <div class="search">
      <svg viewBox="0 0 24 24" width="14" height="14" aria-hidden="true">
        <circle cx="10.5" cy="10.5" r="6.5" fill="none" stroke="currentColor" stroke-width="2" />
        <path d="m15.5 15.5 5 5" stroke="currentColor" stroke-width="2" stroke-linecap="round" />
      </svg>
      <input
        bind:this={input}
        type="search"
        placeholder="Find by file id, asset name, or #MFT index  (Ctrl+F)"
        bind:value={query}
        onkeydown={onKey}
        oninput={() => !query && clearSearch()}
      />
    </div>
    <span class="path mono" title={browser.summary.path}><bdi>{browser.summary.path}</bdi></span>
  {/if}
</header>

<style>
  header {
    display: flex;
    align-items: center;
    gap: 12px;
    height: 48px;
    padding: 0 12px;
    background: var(--panel);
    border-bottom: 1px solid var(--border);
    flex: none;
  }

  .brand {
    display: flex;
    align-items: center;
    gap: 8px;
    font-weight: 600;
    letter-spacing: 0.01em;
    margin-right: 4px;
  }

  .search {
    position: relative;
    display: flex;
    align-items: center;
    color: var(--muted);
  }

  .search svg {
    position: absolute;
    left: 9px;
    pointer-events: none;
  }

  .search input {
    width: 320px;
    padding-left: 28px;
  }

  .path {
    margin-left: auto;
    color: var(--muted);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    direction: rtl;
    max-width: 40%;
  }
</style>
