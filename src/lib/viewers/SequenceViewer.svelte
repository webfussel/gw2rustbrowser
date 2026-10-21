<script lang="ts">
  import { api, type AnimSequence, type SequenceVariant } from "../api";
  import { errorMessage } from "../format";

  let { mft }: { mft: number } = $props();

  const ROW = 28;
  let sequences = $state.raw<AnimSequence[] | null>(null);
  let error = $state<string | null>(null);
  let filter = $state("");
  let selected = $state(0);
  let scrollTop = $state(0);
  let listHeight = $state(400);

  $effect(() => {
    let stale = false;
    api
      .loadSequences(mft)
      .then((s) => !stale && (sequences = s))
      .catch((e) => !stale && (error = errorMessage(e)));
    return () => (stale = true);
  });

  /** Indices of sequences whose name or any variant name matches. */
  const visible = $derived.by(() => {
    if (!sequences) return [];
    const needle = filter.trim().toLowerCase();
    const all = sequences.map((_, i) => i);
    if (!needle) return all;
    return all.filter((i) => {
      const s = sequences![i];
      return s.name.includes(needle) || s.variants.some((v) => v.name.includes(needle));
    });
  });
  const first = $derived(Math.max(0, Math.floor(scrollTop / ROW) - 6));
  const last = $derived(Math.min(visible.length, Math.ceil((scrollTop + listHeight) / ROW) + 6));
  const sequence = $derived(sequences?.[selected] ?? null);
  const variantCount = $derived(sequences?.reduce((n, s) => n + s.variants.length, 0) ?? 0);

  const duration = (v: SequenceVariant) => v.steps.reduce((t, s) => t + s.durationMs, 0);
  /** Timeline length: the steps, or a trigger beyond them. */
  const span = (v: SequenceVariant) => Math.max(1, duration(v), ...v.triggers.map((t) => t.timeMs));
  const seconds = (ms: number) => `${(ms / 1000).toFixed(ms < 1000 ? 3 : 2)} s`;
  /** Distinct colour per trigger kind (their meaning is unknown). */
  const triggerColor = (kind: number) => `hsl(${(kind * 47) % 360} 75% 60%)`;
  const speed = (s: number) => (s === 1 ? "" : `×${Number(s.toFixed(2))}`);

  /** Label row per trigger so labels of close triggers do not overlap. */
  function triggerRows(v: SequenceVariant, total: number): number[] {
    const order = v.triggers.map((t, i) => [t.timeMs, i] as const).sort((a, b) => a[0] - b[0]);
    const rowEnds: number[] = [];
    const rows = new Array<number>(v.triggers.length).fill(0);
    for (const [time, i] of order) {
      const position = time / total;
      let row = rowEnds.findIndex((end) => end <= position);
      if (row < 0) row = rowEnds.push(0) - 1;
      rowEnds[row] = position + 0.035;
      rows[i] = row;
    }
    return rows;
  }
</script>

{#if error}
  <div class="message error">{error}</div>
{:else if !sequences}
  <div class="message muted">Loading…</div>
{:else}
  <div class="layout">
    <aside>
      <input type="search" placeholder="Filter sequences or variants…" bind:value={filter} />
      <div class="count muted">{visible.length.toLocaleString()} of {sequences.length.toLocaleString()} sequences · {variantCount.toLocaleString()} variants</div>
      <div class="list" bind:clientHeight={listHeight} onscroll={(e) => (scrollTop = e.currentTarget.scrollTop)}>
        <div class="spacer" style:height="{visible.length * ROW}px">
          {#each visible.slice(first, last) as index, i (index)}
            <button
              class="row"
              class:active={index === selected}
              style:transform="translateY({(first + i) * ROW}px)"
              onclick={() => (selected = index)}
            >
              <span class="mono">{sequences[index].name}</span>
              <span class="muted">{sequences[index].variants.length}</span>
            </button>
          {/each}
        </div>
      </div>
    </aside>

    <section class="detail">
      {#if sequence}
        <header>
          <h2 class="mono">{sequence.name}</h2>
          <span class="muted">{sequence.variants.length} variant{sequence.variants.length === 1 ? "" : "s"}</span>
          <span class="legend muted">
            <span class="swatch action"></span>action step
            <span class="swatch move"></span>movement step
            <span class="tick"></span>trigger (kind)
          </span>
        </header>
        {#each sequence.variants as v, vi (vi)}
          {@const total = span(v)}
          {@const rows = triggerRows(v, total)}
          {@const rowCount = Math.max(1, ...rows.map((r) => r + 1))}
          <article>
            <div class="title">
              <strong class="mono">{v.name || "(default)"}</strong>
              <span class="muted">
                {seconds(duration(v))} · blend {v.blendIn.toFixed(2)} s in / {v.blendOut.toFixed(2)} s out{v.flags ? ` · flags ${v.flags}` : ""}
              </span>
            </div>

            <div class="timeline" style:margin-bottom="{14 + rowCount * 12}px">
              <div class="steps">
                {#each v.steps as step, si (si)}
                  <div
                    class="step"
                    class:move={step.kind === 1}
                    style:width="{(step.durationMs / total) * 100}%"
                    title="Step {si + 1}: {step.kind === 1 ? 'movement' : 'action'}, {seconds(step.durationMs)}{speed(step.animationSpeed)
                      ? `, animation ${speed(step.animationSpeed)}`
                      : ''}"
                  >
                    <span>{seconds(step.durationMs)} {speed(step.animationSpeed)}</span>
                  </div>
                {/each}
              </div>
              <div class="triggers">
                {#each v.triggers as t, ti (ti)}
                  <div
                    class="trigger"
                    style:left="{(t.timeMs / total) * 100}%"
                    style:--c={triggerColor(t.kind)}
                    style:bottom="{-14 - rows[ti] * 12}px"
                    title="Trigger kind {t.kind} at {seconds(t.timeMs)}{t.flags ? `, flags ${t.flags}` : ''}"
                  >
                    <span>{t.kind}</span>
                  </div>
                {/each}
              </div>
            </div>

            <table>
              <thead>
                <tr><th>#</th><th>Step</th><th>Duration</th><th>Speed</th><th>Velocity</th><th>Flags</th></tr>
              </thead>
              <tbody>
                {#each v.steps as step, si (si)}
                  <tr>
                    <td class="muted">{si + 1}</td>
                    <td>{step.kind === 1 ? "Movement" : "Action"}</td>
                    <td class="mono">{seconds(step.durationMs)}</td>
                    <td class="mono">{step.animationSpeed === 1 ? "1" : Number(step.animationSpeed.toFixed(3))}</td>
                    <td class="mono">{step.velocity ? step.velocity.map((x) => Number(x.toFixed(1))).join(", ") : ""}</td>
                    <td class="mono muted">{step.flags || ""}</td>
                  </tr>
                {/each}
              </tbody>
            </table>
            {#if v.triggers.length}
              <div class="list-line">
                <span class="muted">Triggers:</span>
                {#each v.triggers as t, ti (ti)}
                  <span class="chip" style:--c={triggerColor(t.kind)}>kind {t.kind} @ {seconds(t.timeMs)}</span>
                {/each}
              </div>
            {/if}
            {#if v.chargeStages.length}
              <div class="list-line">
                <span class="muted">Charge stages:</span>
                {#each v.chargeStages as c, ci (ci)}
                  <span class="chip">{c.duration} → level {c.level}</span>
                {/each}
              </div>
            {/if}
          </article>
        {/each}
        <p class="note muted">Times are in seconds from the start of the variant. What the trigger kinds and flags mean is not known.</p>
      {/if}
    </section>
  </div>
{/if}

<style>
  .layout {
    flex: 1;
    display: flex;
    min-height: 0;
  }

  aside {
    width: 240px;
    flex: none;
    display: flex;
    flex-direction: column;
    background: var(--panel);
    border-right: 1px solid var(--border);
  }

  aside input {
    margin: 10px 10px 4px;
  }

  .count {
    padding: 0 12px 8px;
    font-size: 11px;
  }

  .list {
    flex: 1;
    overflow: auto;
  }

  .spacer {
    position: relative;
  }

  .row {
    position: absolute;
    left: 0;
    right: 0;
    height: 28px;
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 0 12px;
    border: 0;
    background: none;
    cursor: pointer;
    text-align: left;
  }

  .row:hover {
    background: var(--row-hover);
  }

  .row.active {
    background: var(--accent-soft);
    box-shadow: inset 2px 0 var(--accent);
  }

  .detail {
    flex: 1;
    min-width: 0;
    overflow: auto;
    padding: 16px 20px 24px;
  }

  header {
    display: flex;
    align-items: baseline;
    gap: 12px;
    margin-bottom: 12px;
    flex-wrap: wrap;
  }

  h2 {
    margin: 0;
    font-size: 18px;
  }

  .legend {
    margin-left: auto;
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 11px;
  }

  .swatch {
    width: 12px;
    height: 10px;
    border-radius: 2px;
    margin-left: 8px;
  }

  .swatch.action,
  .step {
    background: color-mix(in srgb, var(--tone-animation) 55%, var(--panel-3));
  }

  .swatch.move,
  .step.move {
    background: color-mix(in srgb, var(--tone-map) 55%, var(--panel-3));
  }

  .tick {
    width: 2px;
    height: 12px;
    margin-left: 8px;
    background: var(--text);
  }

  article {
    padding: 12px 14px;
    margin-bottom: 12px;
    border: 1px solid var(--border);
    border-radius: 8px;
    background: var(--panel);
  }

  .title {
    display: flex;
    align-items: baseline;
    gap: 12px;
    margin-bottom: 10px;
    flex-wrap: wrap;
  }

  .timeline {
    position: relative;
  }

  .steps {
    display: flex;
    height: 26px;
    border-radius: 5px;
    overflow: hidden;
    background: var(--panel-3);
  }

  .step {
    min-width: 2px;
    border-right: 1px solid var(--panel);
    display: flex;
    align-items: center;
    padding: 0 6px;
    overflow: hidden;
    font-size: 11px;
    white-space: nowrap;
    color: var(--text);
  }

  .triggers {
    position: absolute;
    inset: 0;
    pointer-events: none;
  }

  .trigger {
    position: absolute;
    top: -3px;
    width: 2px;
    margin-left: -1px;
    background: var(--c);
    pointer-events: auto;
  }

  .trigger span {
    position: absolute;
    bottom: -2px;
    left: 3px;
    font-size: 10px;
    color: var(--c);
  }

  table {
    width: 100%;
    border-collapse: collapse;
    font-size: 12px;
  }

  th {
    text-align: left;
    font-size: 11px;
    font-weight: 600;
    color: var(--muted);
    padding: 3px 8px 3px 0;
    border-bottom: 1px solid var(--border);
  }

  td {
    padding: 3px 8px 3px 0;
  }

  .list-line {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px;
    margin-top: 8px;
    font-size: 12px;
  }

  .chip {
    padding: 1px 7px;
    border-radius: 999px;
    background: var(--panel-3);
    border-left: 3px solid var(--c, var(--faint));
    font-family: var(--mono);
    font-size: 11px;
  }

  .note {
    font-size: 12px;
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
