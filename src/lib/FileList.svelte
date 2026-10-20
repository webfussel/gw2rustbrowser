<script lang="ts">
  import type { FileInfo, FindResult } from "./api";
  import { displayName, formatBytes } from "./format";
  import { browser, clearSearch, exportMany, revealCategory } from "./state.svelte";
  import FileIcon from "./FileIcon.svelte";

  const ROW = 26;
  type SortKey = "name" | "fileId" | "type" | "size";

  let sortKey = $state<SortKey>("name");
  let ascending = $state(true);
  let viewport: HTMLDivElement | undefined = $state();
  let scrollTop = $state(0);
  let height = $state(400);

  const searching = $derived(browser.searchResults !== null);

  const rows = $derived.by(() => {
    const list = [...browser.visibleFiles];
    const dir = ascending ? 1 : -1;
    const value = (f: FileInfo) =>
      sortKey === "name" ? f.baseId || -f.mft : sortKey === "fileId" ? f.fileId : sortKey === "size" ? f.size : 0;
    if (sortKey === "type") list.sort((a, b) => dir * a.type.localeCompare(b.type) || a.baseId - b.baseId);
    else list.sort((a, b) => dir * (value(a) - value(b)));
    return list;
  });

  const first = $derived(Math.max(0, Math.floor(scrollTop / ROW) - 8));
  const last = $derived(Math.min(rows.length, Math.ceil((scrollTop + height) / ROW) + 8));
  const selectedIndex = $derived(rows.findIndex((f) => f.mft === browser.selectedMft));

  const title = $derived(
    searching
      ? `Search “${browser.searchQuery}”`
      : browser.selectedCategory !== null
        ? browser.categoryPath(browser.selectedCategory).join(" / ")
        : "Files",
  );

  // Scroll back to the top whenever the list itself changes.
  $effect(() => {
    void browser.visibleFiles;
    if (viewport) viewport.scrollTop = 0;
  });

  function sortBy(key: SortKey) {
    if (sortKey === key) ascending = !ascending;
    else {
      sortKey = key;
      ascending = true;
    }
  }

  function select(index: number) {
    const file = rows[index];
    if (!file) return;
    browser.selectedMft = file.mft;
    if (!viewport) return;
    const top = index * ROW;
    if (top < viewport.scrollTop) viewport.scrollTop = top;
    else if (top + ROW > viewport.scrollTop + height - ROW) viewport.scrollTop = top - height + 2 * ROW;
  }

  function key(event: KeyboardEvent) {
    const page = Math.max(1, Math.floor(height / ROW) - 1);
    const moves: Record<string, number> = {
      ArrowDown: 1,
      ArrowUp: -1,
      PageDown: page,
      PageUp: -page,
      Home: -Infinity,
      End: Infinity,
    };
    if (!(event.key in moves)) return;
    event.preventDefault();
    const target = Math.min(rows.length - 1, Math.max(0, (selectedIndex < 0 ? -1 : selectedIndex) + moves[event.key]));
    select(target);
  }

  function open(file: FileInfo | FindResult) {
    if ("category" in file) {
      clearSearch();
      revealCategory(file.category).then(() => (browser.selectedMft = file.mft));
    }
  }

  function exportList(converted: boolean) {
    if (searching) exportMany({ files: rows.map((f) => f.mft) }, converted);
    else if (browser.selectedCategory !== null) exportMany({ category: browser.selectedCategory }, converted);
  }
</script>

<div class="head">
  <div class="title" title={title}>{title}</div>
  <span class="muted count">{rows.length.toLocaleString()}</span>
  {#if searching}
    <button class="btn small ghost" onclick={clearSearch}>Close</button>
  {/if}
</div>

<div class="columns" class:search={searching}>
  <button onclick={() => sortBy("name")} class:active={sortKey === "name"}>
    ID {#if sortKey === "name"}<span>{ascending ? "▲" : "▼"}</span>{/if}
  </button>
  {#if !searching}
    <button onclick={() => sortBy("fileId")} class:active={sortKey === "fileId"}>
      File ID {#if sortKey === "fileId"}<span>{ascending ? "▲" : "▼"}</span>{/if}
    </button>
  {/if}
  <button onclick={() => sortBy("type")} class:active={sortKey === "type"}>
    Type {#if sortKey === "type"}<span>{ascending ? "▲" : "▼"}</span>{/if}
  </button>
  <button onclick={() => sortBy("size")} class:active={sortKey === "size"} class="right">
    Size {#if sortKey === "size"}<span>{ascending ? "▲" : "▼"}</span>{/if}
  </button>
  {#if searching}<span class="colname">Category</span>{/if}
</div>

<div
  class="viewport"
  bind:this={viewport}
  bind:clientHeight={height}
  onscroll={() => (scrollTop = viewport?.scrollTop ?? 0)}
  onkeydown={key}
  tabindex="0"
  role="listbox"
  aria-label="Files"
>
  {#if rows.length === 0}
    <div class="empty muted">
      {searching ? "No matches." : browser.selectedCategory === null ? "Select a category on the left." : "No files directly in this category."}
    </div>
  {/if}
  <div class="spacer" style:height="{rows.length * ROW}px">
    {#each rows.slice(first, last) as file, i (file.mft)}
      {@const index = first + i}
      <div
        class="row"
        class:search={searching}
        class:selected={file.mft === browser.selectedMft}
        style:transform="translateY({index * ROW}px)"
        role="option"
        aria-selected={file.mft === browser.selectedMft}
        tabindex="-1"
        onclick={() => {
          select(index);
          viewport?.focus();
        }}
        ondblclick={() => open(file)}
        onkeydown={() => {}}
      >
        <span class="name">
          <FileIcon type={file.type} size={14} />
          <span class="mono" title={file.knownName}>{displayName(file)}{#if file.knownName}<span class="known"> · {file.knownName}</span>{/if}</span>
        </span>
        {#if !searching}<span class="mono muted">{file.fileId || ""}</span>{/if}
        <span class="type">{file.type}</span>
        <span class="mono muted right">{formatBytes(file.size)}</span>
        {#if "path" in file}
          <span class="path muted" title={file.path.join(" / ")}>{file.path.join(" / ")}</span>
        {/if}
      </div>
    {/each}
  </div>
</div>

<div class="foot">
  <button class="btn small" disabled={rows.length === 0} onclick={() => exportList(true)} title="Export all files, converted (PNG, …)">
    Export all…
  </button>
  <button class="btn small ghost" disabled={rows.length === 0} onclick={() => exportList(false)} title="Export all files as raw data">
    Raw
  </button>
  {#if searching}
    <span class="muted hint">Double-click: show in category</span>
  {/if}
</div>

<style>
  .head {
    display: flex;
    align-items: center;
    gap: 8px;
    height: 34px;
    padding: 0 8px 0 12px;
    border-bottom: 1px solid var(--border);
    flex: none;
  }

  .title {
    flex: 1;
    font-weight: 600;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .count {
    font-variant-numeric: tabular-nums;
    font-size: 12px;
  }

  .columns,
  .row {
    display: grid;
    grid-template-columns: 1.1fr 1fr 0.9fr 0.8fr;
    gap: 8px;
    align-items: center;
    padding: 0 12px;
  }

  .columns.search,
  .row.search {
    grid-template-columns: 0.9fr 0.7fr 0.8fr 1.8fr;
  }

  .colname {
    font-size: 11px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    color: var(--muted);
  }

  .columns {
    height: 28px;
    border-bottom: 1px solid var(--border);
    flex: none;
    background: var(--panel-2);
  }

  .columns button {
    border: 0;
    background: none;
    padding: 0;
    text-align: left;
    font-size: 11px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    color: var(--muted);
    cursor: pointer;
  }

  .columns button.active {
    color: var(--text);
  }

  .columns button span {
    font-size: 8px;
    margin-left: 2px;
  }

  .right {
    text-align: right !important;
  }

  .viewport {
    flex: 1;
    overflow: auto;
    position: relative;
    outline: none;
  }

  .spacer {
    position: relative;
  }

  .row {
    position: absolute;
    left: 0;
    right: 0;
    height: 26px;
    cursor: default;
    white-space: nowrap;
  }

  .row > span {
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .row:hover {
    background: var(--row-hover);
  }

  .row:focus {
    outline: none;
  }

  .row.selected {
    background: var(--accent-soft);
    box-shadow: inset 2px 0 var(--accent);
  }

  .viewport:focus-visible .row.selected {
    background: color-mix(in srgb, var(--accent) 28%, transparent);
  }

  .path {
    font-size: 11px;
  }

  .type {
    font-size: 12px;
  }

  .name {
    display: flex;
    align-items: center;
    gap: 7px;
    min-width: 0;
  }

  .name > .mono {
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .known {
    color: var(--accent);
    font-family: var(--sans);
  }

  .empty {
    position: absolute;
    inset: 0;
    display: grid;
    place-items: center;
    text-align: center;
    padding: 24px;
  }

  .foot {
    display: flex;
    align-items: center;
    gap: 6px;
    height: 38px;
    padding: 0 8px;
    border-top: 1px solid var(--border);
    flex: none;
  }

  .hint {
    font-size: 11px;
    margin-left: 4px;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
</style>
