<script lang="ts">
  import { onMount } from "svelte";
  import { getCurrentWebview } from "@tauri-apps/api/webview";
  import { api } from "./lib/api";
  import { browser, openArchive } from "./lib/state.svelte";
  import Toolbar from "./lib/Toolbar.svelte";
  import CategoryTree from "./lib/CategoryTree.svelte";
  import FileList from "./lib/FileList.svelte";
  import Preview from "./lib/Preview.svelte";
  import StatusBar from "./lib/StatusBar.svelte";
  import Welcome from "./lib/Welcome.svelte";
  import TaskOverlay from "./lib/TaskOverlay.svelte";
  import Splitter from "./lib/Splitter.svelte";

  let sidebarWidth = $state(300);
  let listWidth = $state(380);

  onMount(() => {
    let unlisten: (() => void) | undefined;
    getCurrentWebview()
      .onDragDropEvent((event) => {
        if (event.payload.type === "drop" && event.payload.paths.length > 0) {
          openArchive(event.payload.paths[0]);
        }
      })
      .then((fn) => (unlisten = fn));
    api.startupPath().then((path) => {
      if (path) openArchive(path);
    });
    return () => unlisten?.();
  });
</script>

<div class="app">
  <Toolbar />
  {#if browser.summary}
    <main>
      <aside style:width="{sidebarWidth}px">
        <CategoryTree />
      </aside>
      <Splitter bind:size={sidebarWidth} min={180} max={600} />
      <section class="list" style:width="{listWidth}px">
        <FileList />
      </section>
      <Splitter bind:size={listWidth} min={260} max={900} />
      <section class="preview">
        <Preview />
      </section>
    </main>
  {:else}
    <Welcome />
  {/if}
  <StatusBar />
  {#if browser.task}
    <TaskOverlay task={browser.task} />
  {/if}
</div>

<style>
  .app {
    display: flex;
    flex-direction: column;
    height: 100%;
  }

  main {
    flex: 1;
    display: flex;
    min-height: 0;
  }

  aside,
  .list {
    flex: none;
    min-width: 0;
    background: var(--panel);
    display: flex;
    flex-direction: column;
  }

  .preview {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    background: var(--bg);
  }
</style>
