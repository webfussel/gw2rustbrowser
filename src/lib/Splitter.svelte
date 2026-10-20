<script lang="ts">
  let { size = $bindable(), min, max }: { size: number; min: number; max: number } = $props();

  function start(event: PointerEvent) {
    const handle = event.currentTarget as HTMLElement;
    handle.setPointerCapture(event.pointerId);
    const startX = event.clientX;
    const startSize = size;
    const move = (e: PointerEvent) => (size = Math.min(max, Math.max(min, startSize + e.clientX - startX)));
    const stop = () => {
      handle.removeEventListener("pointermove", move);
      handle.removeEventListener("pointerup", stop);
    };
    handle.addEventListener("pointermove", move);
    handle.addEventListener("pointerup", stop);
  }
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="splitter" onpointerdown={start}></div>

<style>
  .splitter {
    flex: none;
    width: 5px;
    margin: 0 -2px;
    z-index: 1;
    cursor: col-resize;
    position: relative;
  }

  .splitter::after {
    content: "";
    position: absolute;
    inset: 0 2px;
    background: var(--border);
  }

  .splitter:hover::after {
    background: var(--accent);
    inset: 0 1px;
  }
</style>
