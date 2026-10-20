<script lang="ts">
  import { api } from "../api";
  import { errorMessage } from "../format";

  let { mft }: { mft: number } = $props();

  const ROW = 20;
  const WIDTH = 16;

  let bytes = $state.raw<Uint8Array | null>(null);
  let error = $state<string | null>(null);
  let scrollTop = $state(0);
  let height = $state(400);

  $effect(() => {
    let stale = false;
    api
      .readFile(mft)
      .then((b) => !stale && (bytes = b))
      .catch((e) => !stale && (error = errorMessage(e)));
    return () => (stale = true);
  });

  const rowCount = $derived(bytes ? Math.ceil(bytes.length / WIDTH) : 0);
  const first = $derived(Math.max(0, Math.floor(scrollTop / ROW) - 4));
  const last = $derived(Math.min(rowCount, Math.ceil((scrollTop + height) / ROW) + 4));
  const offsetDigits = $derived(Math.max(8, (bytes?.length ?? 0).toString(16).length));

  const hex = Array.from({ length: 256 }, (_, i) => i.toString(16).padStart(2, "0"));

  function row(index: number) {
    const start = index * WIDTH;
    const slice = bytes!.subarray(start, start + WIDTH);
    let h = "";
    let a = "";
    for (let i = 0; i < WIDTH; i++) {
      if (i < slice.length) {
        h += hex[slice[i]] + (i === 7 ? "  " : " ");
        a += slice[i] >= 0x20 && slice[i] < 0x7f ? String.fromCharCode(slice[i]) : "·";
      } else {
        h += i === 7 ? "    " : "   ";
      }
    }
    return { offset: start.toString(16).padStart(offsetDigits, "0"), hex: h, ascii: a };
  }
</script>

{#if error}
  <div class="message error">{error}</div>
{:else if !bytes}
  <div class="message muted">Loading…</div>
{:else}
  <div class="viewport mono" bind:clientHeight={height} onscroll={(e) => (scrollTop = e.currentTarget.scrollTop)}>
    <div class="spacer" style:height="{rowCount * ROW}px">
      {#each { length: last - first } as _, i (first + i)}
        {@const r = row(first + i)}
        <div class="row" style:transform="translateY({(first + i) * ROW}px)">
          <span class="off">{r.offset}</span><span class="hex">{r.hex}</span><span class="ascii">{r.ascii}</span>
        </div>
      {/each}
    </div>
  </div>
{/if}

<style>
  .viewport {
    flex: 1;
    overflow: auto;
    user-select: text;
    background: var(--panel);
  }

  .spacer {
    position: relative;
    min-width: max-content;
  }

  .row {
    position: absolute;
    height: 20px;
    line-height: 20px;
    padding: 0 16px;
    white-space: pre;
  }

  .off {
    color: var(--faint);
    margin-right: 18px;
  }

  .hex {
    margin-right: 14px;
  }

  .ascii {
    color: var(--muted);
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
