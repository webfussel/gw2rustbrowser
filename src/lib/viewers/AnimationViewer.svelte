<script lang="ts">
  import { PersonStanding } from "@lucide/svelte";
  import { api, type AnimationInfo } from "../api";
  import { errorMessage, formatBytes } from "../format";

  let { mft }: { mft: number } = $props();

  let info = $state<AnimationInfo | null>(null);
  let error = $state<string | null>(null);

  $effect(() => {
    let stale = false;
    api
      .loadAnimation(mft)
      .then((a) => !stale && (info = a))
      .catch((e) => !stale && (error = errorMessage(e)));
    return () => (stale = true);
  });

  const fps = $derived(info && info.timeStep > 0 ? Math.round(1 / info.timeStep) : null);
</script>

<div class="wrap">
  {#if error}
    <p class="error">{error}</p>
  {:else if !info}
    <p class="muted">Loading…</p>
  {:else}
    <div class="card">
      <div class="title">
        <span class="badge"><PersonStanding size={28} strokeWidth={1.6} /></span>
        <div>
          <h2 class="mono">{info.name || "(unnamed clip)"}</h2>
          <p class="muted">Granny 3D animation clip</p>
        </div>
      </div>
      <dl>
        <dt>Duration</dt>
        <dd>{info.duration.toFixed(3)} s</dd>
        <dt>Frames</dt>
        <dd>{info.frames}{fps ? ` at ${fps} fps` : ""}</dd>
        <dt>Oversampling</dt>
        <dd>{info.oversampling}×</dd>
        <dt>Track groups</dt>
        <dd>{info.trackGroups}</dd>
        <dt>Animation data</dt>
        <dd>{formatBytes(info.dataSize)}, {info.relocations.toLocaleString()} pointer fix-ups</dd>
        <dt>Container</dt>
        <dd>MODL file with a single ANIM chunk (v{info.chunkVersion}), {info.pointerBits}-bit offsets</dd>
      </dl>
      <p class="note muted">
        This file holds one animation clip and no geometry. The game plays it on the skeleton of the models that reference it.
      </p>
    </div>
  {/if}
</div>

<style>
  .wrap {
    flex: 1;
    overflow: auto;
    display: grid;
    place-items: center;
    padding: 24px;
  }

  .card {
    width: min(520px, 100%);
    padding: 22px 24px;
    border: 1px solid var(--border);
    border-radius: 10px;
    background: var(--panel);
  }

  .title {
    display: flex;
    align-items: center;
    gap: 14px;
    margin-bottom: 18px;
  }

  .badge {
    display: grid;
    place-items: center;
    width: 48px;
    height: 48px;
    border-radius: 12px;
    color: var(--tone-animation);
    background: color-mix(in srgb, var(--tone-animation) 14%, transparent);
  }

  h2 {
    margin: 0;
    font-size: 17px;
    white-space: pre;
    user-select: text;
  }

  .title p {
    margin: 2px 0 0;
  }

  dl {
    display: grid;
    grid-template-columns: max-content 1fr;
    gap: 8px 18px;
    margin: 0;
  }

  dt {
    color: var(--muted);
  }

  dd {
    margin: 0;
    user-select: text;
  }

  .note {
    margin: 18px 0 0;
    font-size: 12px;
  }

  .error {
    color: var(--accent);
  }
</style>
