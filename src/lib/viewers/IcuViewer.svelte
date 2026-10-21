<script lang="ts">
  import { api, type IcuInfo } from "../api";
  import { errorMessage, formatBytes, formatNumber, naturalCompare } from "../format";

  let { mft }: { mft: number } = $props();

  let info = $state.raw<IcuInfo | null>(null);
  let error = $state<string | null>(null);
  let filter = $state("");
  let group = $state<string | null>(null);

  $effect(() => {
    const id = mft;
    info = null;
    error = null;
    let stale = false;
    api
      .icuInfo(id)
      .then((i) => !stale && (info = i))
      .catch((e) => !stale && (error = errorMessage(e)));
    return () => (stale = true);
  });

  // Entry names are "<package>/<tree>/<item>"; the tree (brkitr, coll, …) groups them.
  const treeOf = (name: string) => {
    const parts = name.split("/");
    return parts.length > 2 ? parts[1] : "(root)";
  };
  const trees = $derived.by(() => {
    const counts = new Map<string, [number, number]>();
    for (const [name, size] of info?.entries ?? []) {
      const t = treeOf(name);
      const [n, bytes] = counts.get(t) ?? [0, 0];
      counts.set(t, [n + 1, bytes + size]);
    }
    return [...counts.entries()].sort((a, b) => naturalCompare(a[0], b[0]));
  });
  const treeNames: Record<string, string> = {
    "(root)": "Locales, converters and properties",
    brkitr: "Line and word breaking",
    coll: "Collation (sorting)",
    curr: "Currency names",
    lang: "Language names",
    rbnf: "Number spelling",
    region: "Region names",
    translit: "Transliteration",
    unit: "Unit names",
    zone: "Time zone names",
  };
  const pkg = $derived(info?.entries[0]?.[0].split("/")[0] ?? "");
  const shown = $derived(
    (info?.entries ?? []).filter(
      ([name]) => (group === null || treeOf(name) === group) && name.toLowerCase().includes(filter.trim().toLowerCase()),
    ),
  );
</script>

{#if error}
  <div class="message error">{error}</div>
{:else if !info}
  <div class="message muted">Loading…</div>
{:else}
  <div class="head">
    <h2>ICU data <span class="mono">{pkg}</span></h2>
    <p class="muted">
      Unicode data of the in-game web browser: locale data, collation, line breaking and character conversion ·
      format {info.formatVersion} · data {info.dataVersion} · {formatNumber(info.entries.length)} entries
    </p>
    {#if info.copyright}<p class="muted small">{info.copyright}</p>{/if}
  </div>
  <div class="bar">
    <input type="search" placeholder="Filter entries" bind:value={filter} />
    <span class="groups">
      <button class="btn small" class:ghost={group !== null} onclick={() => (group = null)}>All</button>
      {#each trees as [t, [n]] (t)}
        <button class="btn small" class:ghost={group !== t} title={treeNames[t] ?? ""} onclick={() => (group = t)}>
          {t} ({n})
        </button>
      {/each}
    </span>
  </div>
  <div class="list">
    <table>
      <thead><tr><th>Entry</th><th class="num">Size</th></tr></thead>
      <tbody>
        {#each shown.slice(0, 3000) as [name, size] (name)}
          <tr><td class="mono">{name.slice(pkg.length + 1)}</td><td class="num">{formatBytes(size)}</td></tr>
        {/each}
      </tbody>
    </table>
  </div>
{/if}

<style>
  .head {
    padding: 14px 16px 10px;
    background: var(--panel);
    flex: none;
    user-select: text;
  }

  h2 {
    margin: 0;
    font-size: 16px;
  }

  .head p {
    margin: 4px 0 0;
  }

  .small {
    font-size: 11px;
  }

  .bar {
    display: flex;
    align-items: center;
    gap: 12px;
    min-height: 44px;
    padding: 4px 12px;
    background: var(--panel);
    border-bottom: 1px solid var(--border);
    flex: none;
    flex-wrap: wrap;
  }

  .groups {
    display: flex;
    flex-wrap: wrap;
    gap: 4px;
  }

  .list {
    flex: 1;
    overflow: auto;
    user-select: text;
  }

  table {
    width: 100%;
    border-collapse: collapse;
  }

  th {
    position: sticky;
    top: 0;
    background: var(--panel);
    text-align: left;
    font-weight: 600;
    color: var(--muted);
  }

  th,
  td {
    padding: 3px 12px;
    border-bottom: 1px solid color-mix(in srgb, var(--border) 60%, transparent);
  }

  .num {
    text-align: right;
    white-space: nowrap;
  }

  .message {
    padding: 24px;
    text-align: center;
  }

  .error {
    color: var(--accent);
  }
</style>
