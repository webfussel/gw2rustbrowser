<script lang="ts">
  import { api } from "./api";
  import type { Task } from "./state.svelte";

  let { task }: { task: Task } = $props();
  let cancelling = $state(false);

  const percent = $derived(task.total > 0 ? Math.min(100, (task.done / task.total) * 100) : null);

  function cancel() {
    cancelling = true;
    api.cancelExport();
  }
</script>

<div class="backdrop">
  <div class="card" role="status" aria-live="polite">
    <div class="label">
      <span>{cancelling ? "Cancelling…" : task.label}</span>
      {#if task.total > 0}
        <span class="mono muted">{task.done.toLocaleString()} / {task.total.toLocaleString()}</span>
      {/if}
    </div>
    <div class="track">
      <div class="fill" class:indeterminate={percent === null} style:width={percent === null ? "30%" : `${percent}%`}></div>
    </div>
    {#if task.cancellable}
      <button class="btn small" onclick={cancel} disabled={cancelling}>Cancel</button>
    {/if}
  </div>
</div>

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    display: grid;
    place-items: center;
    background: rgba(0, 0, 0, 0.35);
    backdrop-filter: blur(2px);
    z-index: 10;
  }

  .card {
    width: min(440px, 90vw);
    padding: 18px 20px;
    border-radius: 10px;
    background: var(--panel);
    border: 1px solid var(--border);
    box-shadow: 0 12px 40px rgba(0, 0, 0, 0.4);
    display: flex;
    flex-direction: column;
    gap: 12px;
    align-items: flex-start;
  }

  .label {
    width: 100%;
    display: flex;
    justify-content: space-between;
    font-weight: 600;
  }

  .track {
    width: 100%;
    height: 6px;
    border-radius: 6px;
    background: var(--panel-3);
    overflow: hidden;
  }

  .fill {
    height: 100%;
    background: var(--accent);
    border-radius: 6px;
    transition: width 0.15s;
  }

  .fill.indeterminate {
    animation: slide 1.1s ease-in-out infinite;
  }

  @keyframes slide {
    from {
      transform: translateX(-100%);
    }
    to {
      transform: translateX(340%);
    }
  }
</style>
