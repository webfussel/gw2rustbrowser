<script lang="ts">
  import { api, type FlashInfo } from "../api";
  import { errorMessage, formatBytes } from "../format";

  let { mft }: { mft: number } = $props();

  let info = $state<FlashInfo | null>(null);
  let error = $state<string | null>(null);

  $effect(() => {
    const id = mft;
    info = null;
    error = null;
    let stale = false;
    api
      .flashInfo(id)
      .then((f) => !stale && (info = f))
      .catch((e) => !stale && (error = errorMessage(e)));
    return () => (stale = true);
  });

  const compression: Record<string, string> = { none: "None", zlib: "zlib (CWS)", LZMA: "LZMA (ZWS)" };
</script>

<div class="wrap">
  {#if error}
    <p class="error">{error}</p>
  {:else if !info}
    <p class="muted">Loading…</p>
  {:else}
    <h2>Flash movie</h2>
    <dl>
      <dt>SWF version</dt>
      <dd>{info.version}</dd>
      <dt>Compression</dt>
      <dd>{compression[info.compression] ?? info.compression}</dd>
      <dt>Uncompressed size</dt>
      <dd>{formatBytes(info.length)}</dd>
      {#if info.width !== null && info.height !== null}
        <dt>Stage</dt>
        <dd>{info.width} × {info.height}</dd>
      {/if}
      {#if info.frameRate !== null}
        <dt>Frame rate</dt>
        <dd>{info.frameRate} fps</dd>
      {/if}
      {#if info.frames !== null}
        <dt>Frames</dt>
        <dd>{info.frames}</dd>
      {/if}
    </dl>
    <p class="note muted">
      A Flash movie shipped with the in-game web browser. Flash Player is discontinued, so it is not played here; export it to open it in a Flash emulator such as Ruffle.
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
