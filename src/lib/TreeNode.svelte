<script lang="ts">
  import { browser, selectCategory } from "./state.svelte";
  import TreeNode from "./TreeNode.svelte";
  import FileIcon from "./FileIcon.svelte";

  let { id, depth }: { id: number; depth: number } = $props();

  const category = $derived(browser.categories[id]);
  const children = $derived(browser.children.get(id) ?? []);
  const open = $derived(browser.expanded.has(id));
  const selected = $derived(browser.selectedCategory === id && !browser.searchResults);

  function toggle(event: MouseEvent) {
    event.stopPropagation();
    if (open) browser.expanded.delete(id);
    else browser.expanded.add(id);
  }

  function choose() {
    selectCategory(id);
    if (children.length > 0 && category.files === 0 && !open) browser.expanded.add(id);
  }

  function key(event: KeyboardEvent) {
    if (event.key === "Enter" || event.key === " ") {
      event.preventDefault();
      choose();
    } else if (event.key === "ArrowRight" && children.length) browser.expanded.add(id);
    else if (event.key === "ArrowLeft") browser.expanded.delete(id);
  }
</script>

<div
  class="node"
  class:selected
  style:padding-left="{8 + depth * 14}px"
  role="treeitem"
  aria-selected={selected}
  aria-expanded={children.length ? open : undefined}
  tabindex="0"
  onclick={choose}
  onkeydown={key}
>
  {#if children.length}
    <button class="twisty" class:open onclick={toggle} tabindex="-1" aria-label={open ? "Collapse" : "Expand"}>
      <svg viewBox="0 0 16 16" width="12" height="12"><path d="M6 4l4 4-4 4" fill="none" stroke="currentColor" stroke-width="1.6" /></svg>
    </button>
  {:else}
    <span class="twisty"></span>
  {/if}
  <FileIcon type={category.content} />
  <span class="name">{category.name}</span>
  <span class="count">{category.total.toLocaleString()}</span>
</div>
{#if open}
  {#each children as child (child)}
    <TreeNode id={child} depth={depth + 1} />
  {/each}
{/if}

<style>
  .node {
    display: flex;
    align-items: center;
    gap: 6px;
    height: 26px;
    padding-right: 10px;
    cursor: default;
    white-space: nowrap;
    border-left: 2px solid transparent;
  }

  .node:hover {
    background: var(--row-hover);
  }

  .node.selected {
    background: var(--accent-soft);
    border-left-color: var(--accent);
  }

  .twisty {
    display: grid;
    place-items: center;
    width: 16px;
    height: 16px;
    padding: 0;
    border: 0;
    background: none;
    color: var(--muted);
    flex: none;
    cursor: pointer;
  }

  .twisty svg {
    transition: transform 0.12s;
  }

  .twisty.open svg {
    transform: rotate(90deg);
  }

  .name {
    flex: 1;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .count {
    color: var(--faint);
    font-size: 11px;
    font-variant-numeric: tabular-nums;
  }
</style>
