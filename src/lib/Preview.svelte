<script lang="ts">
  import { api, type EntryDetail } from "./api";
  import { displayName, errorMessage, formatBytes } from "./format";
  import { browser, clearSearch, exportMany, exportOne, revealCategory } from "./state.svelte";
  import ImageViewer from "./viewers/ImageViewer.svelte";
  import HexViewer from "./viewers/HexViewer.svelte";
  import TextViewer from "./viewers/TextViewer.svelte";
  import SoundPlayer from "./viewers/SoundPlayer.svelte";
  import SoundBankViewer from "./viewers/SoundBankViewer.svelte";
  import StringsViewer from "./viewers/StringsViewer.svelte";
  import EulaViewer from "./viewers/EulaViewer.svelte";
  import FontViewer from "./viewers/FontViewer.svelte";
  import AnimationViewer from "./viewers/AnimationViewer.svelte";
  import FileIcon from "./FileIcon.svelte";
  import ModelSummaryCard from "./viewers/ModelSummaryCard.svelte";
  import TrueTypeViewer from "./viewers/TrueTypeViewer.svelte";
  import GlyphFileViewer from "./viewers/GlyphFileViewer.svelte";

  /** Model types without meshes get a summary instead of the 3D view. */
  const meshlessModels = ["MODL skeleton", "MODL animset", "MODL materials", "MODL empty"];
  // three.js is large; load the 3D viewer only when a model is shown.
  const modelViewer = () => import("./viewers/ModelViewer.svelte");

  type Tab = "preview" | "hex";

  let detail = $state<EntryDetail | null>(null);
  let error = $state<string | null>(null);
  let tab = $state<Tab>("preview");

  const previewKinds = [
    "image",
    "text",
    "content",
    "sound",
    "soundbank",
    "strings",
    "eula",
    "model",
    "animation",
    "bitmapfont",
    "fontglyphs",
    "font",
  ];
  const hasPreview = $derived(detail !== null && previewKinds.includes(detail.kind));
  const activeTab = $derived<Tab>(hasPreview ? tab : "hex");

  $effect(() => {
    const mft = browser.selectedMft;
    error = null;
    // A newly selected file always opens in its preview.
    tab = "preview";
    if (mft === null) {
      detail = null;
      return;
    }
    let stale = false;
    api
      .entryInfo(mft)
      .then((d) => !stale && (detail = d))
      .catch((e) => !stale && (error = errorMessage(e)));
    return () => (stale = true);
  });

  function showCategory() {
    if (!detail) return;
    clearSearch();
    revealCategory(detail.category);
  }

  function copy(text: string) {
    navigator.clipboard.writeText(text);
  }
</script>

{#if detail}
  {@const d = detail}
  <div class="head">
    <div class="titleline">
      <FileIcon type={d.type} size={20} />
      <h1 class="mono">{displayName(d)}</h1>
      {#if d.knownName}<span class="known">{d.knownName}</span>{/if}
      <span class="badge">{d.type}</span>
      <div class="actions">
        {#if d.kind === "soundbank"}
          <button class="btn primary small" onclick={() => exportMany({ files: [d.mft] }, true)}>Export sounds…</button>
        {:else if d.kind === "bitmapfont"}
          <button class="btn primary small" onclick={() => exportMany({ files: [d.mft] }, true)}>Export atlases…</button>
        {:else if d.convertedExtension !== d.rawExtension}
          <button class="btn primary small" onclick={() => exportOne(d.mft, true)}>Export .{d.convertedExtension}</button>
        {/if}
        <button class="btn small" onclick={() => exportOne(d.mft, false)}>Export raw</button>
      </div>
    </div>
    <dl>
      <div><dt>Base ID</dt><dd class="mono"><button class="copy" title="Copy" onclick={() => copy(String(d.baseId))}>{d.baseId || "–"}</button></dd></div>
      <div><dt>File ID</dt><dd class="mono"><button class="copy" title="Copy" onclick={() => copy(String(d.fileId))}>{d.fileId || "–"}</button></dd></div>
      <div><dt>MFT</dt><dd class="mono">#{d.mft}</dd></div>
      <div><dt>Size</dt><dd class="mono">{formatBytes(d.size)}</dd></div>
      <div>
        <dt>Stored</dt>
        <dd class="mono">{formatBytes(d.storedSize)}{d.compressed ? ` (${Math.round((d.storedSize / Math.max(1, d.size)) * 100)} %)` : ""}</dd>
      </div>
    </dl>
    <button class="crumbs" onclick={showCategory} title="Show in category">
      {#each d.path as part, i}
        {#if i > 0}<span class="sep">/</span>{/if}<span>{part}</span>
      {/each}
    </button>
    {#if hasPreview}
      <div class="tabs" role="tablist">
        <button role="tab" aria-selected={activeTab === "preview"} class:active={activeTab === "preview"} onclick={() => (tab = "preview")}>
          Preview
        </button>
        <button role="tab" aria-selected={activeTab === "hex"} class:active={activeTab === "hex"} onclick={() => (tab = "hex")}>Hex</button>
      </div>
    {/if}
  </div>
  <div class="body">
    {#key d.mft}
      {#if activeTab === "preview" && d.kind === "image"}
        <ImageViewer load={() => api.loadImage(d.mft)} />
      {:else if activeTab === "preview" && (d.kind === "text" || d.kind === "content")}
        <TextViewer mft={d.mft} />
      {:else if activeTab === "preview" && d.kind === "sound"}
        <SoundPlayer mft={d.mft} />
      {:else if activeTab === "preview" && d.kind === "soundbank"}
        <SoundBankViewer mft={d.mft} name={displayName(d)} />
      {:else if activeTab === "preview" && d.kind === "strings"}
        <StringsViewer mft={d.mft} />
      {:else if activeTab === "preview" && d.kind === "eula"}
        <EulaViewer mft={d.mft} />
      {:else if activeTab === "preview" && d.kind === "animation"}
        <AnimationViewer mft={d.mft} />
      {:else if activeTab === "preview" && d.kind === "bitmapfont"}
        <FontViewer mft={d.mft} />
      {:else if activeTab === "preview" && d.kind === "font"}
        <TrueTypeViewer mft={d.mft} />
      {:else if activeTab === "preview" && d.kind === "fontglyphs"}
        <GlyphFileViewer mft={d.mft} />
      {:else if activeTab === "preview" && d.kind === "model" && meshlessModels.includes(d.type)}
        <ModelSummaryCard mft={d.mft} type={d.type} />
      {:else if activeTab === "preview" && d.kind === "model"}
        {#await modelViewer() then { default: ModelViewer }}
          <ModelViewer mft={d.mft} />
        {/await}
      {:else}
        <HexViewer mft={d.mft} />
      {/if}
    {/key}
  </div>
{:else if error}
  <div class="placeholder error">{error}</div>
{:else}
  <div class="placeholder muted">Select a file to preview it.</div>
{/if}

<style>
  .head {
    padding: 14px 16px 0;
    background: var(--panel);
    border-bottom: 1px solid var(--border);
    flex: none;
  }

  .titleline {
    display: flex;
    align-items: center;
    gap: 10px;
  }

  h1 {
    margin: 0;
    font-size: 18px;
    font-weight: 600;
    user-select: text;
  }

  .known {
    font-weight: 600;
    color: var(--muted);
  }

  .badge {
    padding: 2px 8px;
    border-radius: 999px;
    background: var(--accent-soft);
    color: var(--accent);
    font-size: 11px;
    font-weight: 600;
  }

  .actions {
    margin-left: auto;
    display: flex;
    gap: 6px;
  }

  dl {
    display: flex;
    flex-wrap: wrap;
    gap: 4px 24px;
    margin: 10px 0 6px;
  }

  dl div {
    display: flex;
    gap: 6px;
    align-items: baseline;
  }

  dt {
    color: var(--muted);
    font-size: 11px;
    text-transform: uppercase;
    letter-spacing: 0.05em;
  }

  dd {
    margin: 0;
  }

  .copy {
    border: 0;
    background: none;
    padding: 0;
    font: inherit;
    cursor: copy;
  }

  .copy:hover {
    color: var(--accent);
  }

  .crumbs {
    display: flex;
    flex-wrap: wrap;
    gap: 4px;
    border: 0;
    background: none;
    padding: 0;
    margin-bottom: 10px;
    color: var(--muted);
    font-size: 12px;
    cursor: pointer;
  }

  .crumbs:hover span:not(.sep) {
    color: var(--text);
    text-decoration: underline;
  }

  .sep {
    color: var(--faint);
  }

  .tabs {
    display: flex;
    gap: 2px;
  }

  .tabs button {
    border: 0;
    background: none;
    padding: 6px 12px;
    color: var(--muted);
    border-bottom: 2px solid transparent;
    cursor: pointer;
  }

  .tabs button.active {
    color: var(--text);
    border-bottom-color: var(--accent);
  }

  .body {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
  }

  .placeholder {
    flex: 1;
    display: grid;
    place-items: center;
    padding: 24px;
  }

  .error {
    color: var(--accent);
  }
</style>
