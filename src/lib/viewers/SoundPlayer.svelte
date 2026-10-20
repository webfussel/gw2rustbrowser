<script lang="ts">
  import { api } from "../api";
  import { errorMessage, formatBytes } from "../format";

  let { mft }: { mft: number } = $props();

  const AUTOPLAY_KEY = "gw2browser.autoplay";
  function readAutoplay() {
    try {
      return localStorage.getItem(AUTOPLAY_KEY) === "1";
    } catch {
      return false;
    }
  }

  let url = $state<string | null>(null);
  let info = $state<{ mime: string; size: number } | null>(null);
  let error = $state<string | null>(null);
  let autoplay = $state(readAutoplay());

  $effect(() => {
    let stale = false;
    let created: string | null = null;
    api
      .loadSound(mft)
      .then((sound) => {
        if (stale) return;
        created = URL.createObjectURL(new Blob([sound.data], { type: sound.mime }));
        url = created;
        info = { mime: sound.mime, size: sound.data.length };
      })
      .catch((e) => !stale && (error = errorMessage(e)));
    return () => {
      stale = true;
      if (created) URL.revokeObjectURL(created);
    };
  });

  function setAutoplay(value: boolean) {
    autoplay = value;
    try {
      localStorage.setItem(AUTOPLAY_KEY, value ? "1" : "0");
    } catch {
      // Storage may be unavailable; the setting then lasts for this session.
    }
  }
</script>

<div class="player">
  {#if error}
    <p class="error">{error}</p>
  {:else if url && info}
    <svg class="wave" viewBox="0 0 120 40" aria-hidden="true">
      {#each Array.from({ length: 30 }) as _, i}
        {@const h = 6 + Math.abs(Math.sin(i * 1.7 + mft) * 28)}
        <rect x={i * 4} y={20 - h / 2} width="2.4" height={h} rx="1.2" />
      {/each}
    </svg>
    <audio controls src={url} autoplay={autoplay}></audio>
    <div class="meta muted mono">{info.mime === "audio/mpeg" ? "MP3" : "Ogg Vorbis"} · {formatBytes(info.size)}</div>
    <label class="muted"><input type="checkbox" checked={autoplay} onchange={(e) => setAutoplay(e.currentTarget.checked)} /> Play automatically when selected</label>
  {:else}
    <p class="muted">Loading…</p>
  {/if}
</div>

<style>
  .player {
    flex: 1;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 14px;
    padding: 24px;
  }

  .wave {
    width: 240px;
    fill: var(--accent);
    opacity: 0.8;
  }

  audio {
    width: min(520px, 90%);
  }

  .error {
    color: var(--accent);
  }
</style>
