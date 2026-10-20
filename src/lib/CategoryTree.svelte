<script lang="ts">
  import { browser } from "./state.svelte";
  import TreeNode from "./TreeNode.svelte";

  const roots = $derived(browser.children.get(-1) ?? []);
</script>

<div class="head">
  <span>Categories</span>
  <span class="muted">{browser.categories.length.toLocaleString()}</span>
</div>
<div class="tree" role="tree">
  {#each roots as id (id)}
    <TreeNode {id} depth={0} />
  {/each}
</div>

<style>
  .head {
    display: flex;
    justify-content: space-between;
    align-items: center;
    height: 34px;
    padding: 0 12px;
    font-size: 11px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    color: var(--muted);
    border-bottom: 1px solid var(--border);
    flex: none;
  }

  .tree {
    flex: 1;
    overflow: auto;
    padding: 4px 0 12px;
  }
</style>
