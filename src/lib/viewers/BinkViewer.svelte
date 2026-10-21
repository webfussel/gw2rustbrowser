<script lang="ts">
  import { api, type BinkInfo } from "../api";
  import { errorMessage, formatBytes, formatNumber } from "../format";

  let { mft }: { mft: number } = $props();

  let info = $state<BinkInfo | null>(null);
  let error = $state<string | null>(null);

  $effect(() => {
    const id = mft;
    info = null;
    error = null;
    let stale = false;
    api
      .binkInfo(id)
      .then((b) => !stale && (info = b))
      .catch((e) => !stale && (error = errorMessage(e)));
    return () => (stale = true);
  });

  function duration(seconds: number) {
    const m = Math.floor(seconds / 60);
    const s = seconds - m * 60;
    return `${m}:${s.toFixed(1).padStart(4, "0")}`;
  }
</script>

<div class="wrap">
  <div class="notice">
    <strong>This video cannot be played here.</strong>
    <p>
      Bink 2 is a proprietary video codec owned by RAD Game Tools (now part of Epic Games). Its format is not
      published and there is no open decoder for it, so the browser can only read the file header. Export the file
      (it is kept unchanged as <span class="mono">.bk2</span>) and open it in RAD Video Tools to watch it.
    </p>
  </div>

  {#if error}
    <p class="error">{error}</p>
  {:else if !info}
    <p class="muted">Loading…</p>
  {:else}
    <dl>
      <dt>Format</dt>
      <dd>Bink 2, revision <span class="mono">{info.revision}</span></dd>
      <dt>Resolution</dt>
      <dd>{info.width} × {info.height}</dd>
      <dt>Frame rate</dt>
      <dd>{info.fps.toFixed(3).replace(/\.?0+$/, "")} fps</dd>
      <dt>Frames</dt>
      <dd>{formatNumber(info.frames)}</dd>
      <dt>Duration</dt>
      <dd>{duration(info.duration)}</dd>
      <dt>Audio tracks</dt>
      <dd>{info.audioTracks}</dd>
      <dt>Largest frame</dt>
      <dd>{formatBytes(info.largestFrame)}</dd>
      <dt>File size</dt>
      <dd>{formatBytes(info.size)}</dd>
      <dt>Average bit rate</dt>
      <dd>{info.duration > 0 ? `${((info.size * 8) / info.duration / 1e6).toFixed(2)} Mbit/s` : "–"}</dd>
      <dt>Flags</dt>
      <dd class="mono">0x{info.flags.toString(16).padStart(8, "0")}</dd>
    </dl>
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

  .notice {
    max-width: 70ch;
    padding: 12px 14px;
    border: 1px solid color-mix(in srgb, var(--tone-video) 50%, var(--border));
    border-radius: var(--radius);
    background: color-mix(in srgb, var(--tone-video) 10%, transparent);
  }

  .notice p {
    margin: 6px 0 0;
    line-height: 1.5;
  }

  dl {
    display: grid;
    grid-template-columns: max-content 1fr;
    gap: 6px 20px;
    margin: 20px 0 0;
  }

  dt {
    color: var(--muted);
  }

  dd {
    margin: 0;
  }

  .error {
    color: var(--accent);
  }
</style>
