<script lang="ts">
  import { api } from "../api";
  import { errorMessage } from "../format";
  import { highlight } from "../highlight";

  /**
   * `language`: highlight.js language for syntax highlighting, if any.
   * `load`: where the text comes from, instead of the archive file `mft`.
   */
  let {
    mft = 0,
    language = null,
    load = null,
  }: { mft?: number; language?: string | null; load?: (() => Promise<string>) | null } = $props();

  const LIMIT = 4 * 1024 * 1024;
  // Larger files stay plain: highlighting them would freeze the view.
  const HIGHLIGHT_LIMIT = 1024 * 1024;
  let text = $state<string | null>(null);
  let error = $state<string | null>(null);
  let truncated = $state(false);
  let wrap = $state(true);
  let colored = $state(true);
  let highlighted = $state<string | null>(null);

  $effect(() => {
    let stale = false;
    (load ?? (() => api.loadText(mft)))()
      .then((full) => {
        if (stale) return;
        truncated = full.length > LIMIT;
        text = truncated ? full.slice(0, LIMIT) : full;
      })
      .catch((e) => !stale && (error = errorMessage(e)));
    return () => (stale = true);
  });

  const tooLarge = $derived(text !== null && text.length > HIGHLIGHT_LIMIT);

  $effect(() => {
    highlighted = null;
    const source = text;
    if (!language || !colored || source === null || tooLarge) return;
    let stale = false;
    // Show the plain text first; the highlighted version replaces it.
    const timer = setTimeout(() => {
      highlight(source, language)
        .then((html) => !stale && (highlighted = html))
        .catch(() => {});
    }, 0);
    return () => {
      stale = true;
      clearTimeout(timer);
    };
  });

  const languageNames: Record<string, string> = {
    xml: "HTML / XML",
    javascript: "JavaScript",
    css: "CSS",
    json: "JSON",
    php: "PHP",
    markdown: "Markdown",
    actionscript: "ActionScript",
  };
</script>

<div class="bar">
  <label><input type="checkbox" bind:checked={wrap} /> Wrap lines</label>
  {#if language}
    <label title={tooLarge ? "Files over 1 MiB are shown without highlighting" : ""}>
      <input type="checkbox" bind:checked={colored} disabled={tooLarge} /> Highlight {languageNames[language] ?? language}
    </label>
  {/if}
  {#if truncated}<span class="muted">Showing the first 4 million characters</span>{/if}
  {#if tooLarge && language}<span class="muted">Too large to highlight</span>{/if}
</div>
{#if error}
  <div class="message error">{error}</div>
{:else if text === null}
  <div class="message muted">Loading…</div>
{:else if highlighted !== null}
  <!-- highlight.js escapes the text; the markup only adds token spans. -->
  <pre class="mono code" class:wrap>{@html highlighted}</pre>
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

  label {
    display: flex;
    align-items: center;
    gap: 6px;
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

  /* Token colours from the app's palette, so light and dark themes both work. */
  .code :global(.hljs-comment),
  .code :global(.hljs-quote) {
    color: var(--faint);
    font-style: italic;
  }

  .code :global(.hljs-keyword),
  .code :global(.hljs-selector-tag),
  .code :global(.hljs-doctag),
  .code :global(.hljs-section) {
    color: var(--tone-text);
  }

  .code :global(.hljs-string),
  .code :global(.hljs-regexp),
  .code :global(.hljs-addition),
  .code :global(.hljs-template-tag) {
    color: var(--tone-map);
  }

  .code :global(.hljs-number),
  .code :global(.hljs-literal),
  .code :global(.hljs-bullet),
  .code :global(.hljs-symbol) {
    color: var(--tone-model);
  }

  .code :global(.hljs-title),
  .code :global(.hljs-title.function_),
  .code :global(.hljs-selector-id),
  .code :global(.hljs-selector-class) {
    color: var(--tone-font);
  }

  .code :global(.hljs-name),
  .code :global(.hljs-tag) {
    color: var(--tone-sound);
  }

  .code :global(.hljs-attr),
  .code :global(.hljs-attribute),
  .code :global(.hljs-property),
  .code :global(.hljs-selector-attr),
  .code :global(.hljs-selector-pseudo) {
    color: var(--tone-image);
  }

  .code :global(.hljs-built_in),
  .code :global(.hljs-type),
  .code :global(.hljs-variable),
  .code :global(.hljs-params) {
    color: var(--tone-animation);
  }

  .code :global(.hljs-meta),
  .code :global(.hljs-meta .hljs-keyword) {
    color: var(--muted);
  }

  .code :global(.hljs-deletion) {
    color: var(--accent);
  }

  .code :global(.hljs-link) {
    color: var(--tone-text);
    text-decoration: underline;
  }

  .code :global(.hljs-emphasis) {
    font-style: italic;
  }

  .code :global(.hljs-strong),
  .code :global(.hljs-section) {
    font-weight: 600;
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
