<script lang="ts">
  import { api, type ModelSummary } from "../api";
  import FileIcon from "../FileIcon.svelte";
  import { errorMessage } from "../format";
  import { iconFor } from "../icons";
  import { showFile } from "../state.svelte";

  let { mft, type }: { mft: number; type: string } = $props();

  let summary = $state<ModelSummary | null>(null);
  let error = $state<string | null>(null);

  $effect(() => {
    let stale = false;
    api
      .modelSummary(mft)
      .then((s) => !stale && (summary = s))
      .catch((e) => !stale && (error = errorMessage(e)));
    return () => (stale = true);
  });

  const descriptions: Record<string, string> = {
    "MODL skeleton":
      "A skeleton without meshes. Armor, body parts and creature models are bound to skeletons like this one, which usually also carry their animations.",
    "MODL animset": "Animations for another model's skeleton, without meshes of its own.",
    "MODL empty": "This model file contains no meshes and no animations.",
  };
</script>

<div class="wrap">
  {#if error}
    <p class="error">{error}</p>
  {:else if !summary}
    <p class="muted">Loading…</p>
  {:else}
    <div class="card">
      <div class="title">
        <span class="badge"><FileIcon {type} size={28} /></span>
        <div>
          <h2>{iconFor(type).label}</h2>
          <p class="muted">{descriptions[type] ?? ""}</p>
        </div>
      </div>
      <dl>
        <dt>Meshes</dt>
        <dd>{summary.meshes}</dd>
        <dt>Bones</dt>
        <dd>{summary.bones}</dd>
        <dt>Animations</dt>
        <dd>{summary.animations}</dd>
        <dt>Materials</dt>
        <dd>{summary.materials}</dd>
        {#if summary.users > 0}
          <dt>Used by</dt>
          <dd>{summary.users.toLocaleString()} models</dd>
        {/if}
        {#if summary.skeleton}
          <dt>Skeleton</dt>
          <dd><button class="link mono" onclick={() => showFile(summary!.skeleton!)}>{summary.skeleton} →</button></dd>
        {/if}
      </dl>
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
    flex: none;
    border-radius: 12px;
    background: var(--panel-3);
  }

  h2 {
    margin: 0;
    font-size: 17px;
  }

  .title p {
    margin: 4px 0 0;
    font-size: 12px;
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
  }

  .link {
    border: 0;
    background: none;
    padding: 0;
    color: var(--accent);
    cursor: pointer;
  }

  .link:hover {
    text-decoration: underline;
  }

  .error {
    color: var(--accent);
  }
</style>
