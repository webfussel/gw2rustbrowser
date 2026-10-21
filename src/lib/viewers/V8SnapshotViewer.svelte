<script lang="ts">
  import { api, type V8Info } from "../api";
  import { errorMessage, formatBytes } from "../format";

  let { mft }: { mft: number } = $props();

  let info = $state<V8Info | null>(null);
  let error = $state<string | null>(null);

  $effect(() => {
    const id = mft;
    info = null;
    error = null;
    let stale = false;
    api
      .v8Info(id)
      .then((v) => !stale && (info = v))
      .catch((e) => !stale && (error = errorMessage(e)));
    return () => (stale = true);
  });

  // Since V8 7.0 (Chromium 70), Chromium's major version is V8's major and minor run together: V8 10.3 → 103.
  const chromium = $derived.by(() => {
    const [major, minor] = (info?.version ?? "").split(".").map(Number);
    return major >= 7 ? major * 10 + minor : null;
  });
</script>

<div class="wrap">
  {#if error}
    <p class="error">{error}</p>
  {:else if !info}
    <p class="muted">Loading…</p>
  {:else}
    <h2>V8 startup snapshot</h2>
    <dl>
      <dt>V8 version</dt>
      <dd class="mono">{info.version}</dd>
      {#if chromium}
        <dt>Chromium</dt>
        <dd>{chromium}</dd>
      {/if}
      <dt>Contexts</dt>
      <dd>{info.contexts}</dd>
      <dt>Rehashable</dt>
      <dd>{info.rehashable ? "Yes" : "No"}</dd>
      <dt>Checksum</dt>
      <dd class="mono">0x{info.checksum.toString(16).padStart(8, "0")}</dd>
      <dt>Size</dt>
      <dd>{formatBytes(info.size)}</dd>
    </dl>
    <p class="note muted">
      A serialised JavaScript heap that the in-game web browser (Chromium Embedded Framework) loads at startup instead
      of building its built-in objects from scratch. It is machine-specific data with no readable content.
    </p>
  {/if}
</div>

<style>
  .wrap {
    flex: 1;
    overflow: auto;
    padding: 20px 24px;
    background: var(--panel);
    user-select: text;
  }

  h2 {
    margin: 0;
    font-size: 18px;
  }

  dl {
    display: grid;
    grid-template-columns: max-content 1fr;
    gap: 6px 20px;
    margin: 16px 0 0;
  }

  dt {
    color: var(--muted);
  }

  dd {
    margin: 0;
  }

  .note {
    max-width: 70ch;
    margin-top: 20px;
    line-height: 1.5;
  }

  .error {
    color: var(--accent);
  }
</style>
