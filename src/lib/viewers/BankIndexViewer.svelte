<script lang="ts">
  import { api, type BankIndexLanguage } from "../api";
  import { errorMessage } from "../format";
  import SoundBankViewer from "./SoundBankViewer.svelte";

  let { mft }: { mft: number } = $props();

  const ROW = 28;

  let languages = $state.raw<BankIndexLanguage[] | null>(null);
  let error = $state<string | null>(null);
  let language = $state(0);
  let selected = $state<number | null>(null);
  let query = $state("");
  let scrollTop = $state(0);
  let viewport = $state(400);

  $effect(() => {
    const id = mft;
    languages = null;
    error = null;
    selected = null;
    let stale = false;
    api
      .loadBankIndex(id)
      .then((list) => {
        if (stale) return;
        languages = list;
        // The archive only holds the banks of the game's voice language.
        const installed = list.map((l) => l.mfts.filter((m) => m >= 0).length);
        language = installed.indexOf(Math.max(...installed));
        // Open the first bank right away; small indexes hold only one.
        const first = list[language]?.files.findIndex((f) => f !== 0) ?? -1;
        selected = first >= 0 ? first : null;
      })
      .catch((e) => !stale && (error = errorMessage(e)));
    return () => (stale = true);
  });

  /** Bank numbers that have a file in any language, filtered by the search. */
  const rows = $derived.by(() => {
    if (!languages) return [];
    const count = Math.max(0, ...languages.map((l) => l.files.length));
    const needle = query.trim();
    const result: number[] = [];
    for (let bank = 0; bank < count; bank++) {
      if (!languages.some((l) => l.files[bank])) continue;
      if (needle && !String(bank).startsWith(needle) && !languages.some((l) => String(l.files[bank] ?? "").startsWith(needle)))
        continue;
      result.push(bank);
    }
    return result;
  });

  function select(bank: number) {
    selected = bank;
    // Stay in the chosen language if the bank has it, else use the first that does.
    if (languages && !languages[language]?.files[bank]) {
      const other = languages.findIndex((l) => l.files[bank]);
      if (other >= 0) language = other;
    }
  }

  const first = $derived(Math.max(0, Math.floor(scrollTop / ROW) - 10));
  const visible = $derived(rows.slice(first, first + Math.ceil(viewport / ROW) + 20));
  const installedCount = $derived(languages?.map((l) => l.mfts.filter((m) => m >= 0).length) ?? []);
  const current = $derived(languages?.[language] ?? null);
  const bankMft = $derived(selected !== null && current ? (current.mfts[selected] ?? -1) : -1);
  const bankFile = $derived(selected !== null && current ? (current.files[selected] ?? 0) : 0);
</script>

{#if error}
  <div class="message error">{error}</div>
{:else if !languages}
  <div class="message muted">Loading…</div>
{:else if languages.length === 0}
  <div class="message muted">
    This index is empty: it has a slot per language, but none of them names a bank. Sound bank index 184691 lists every
    bank.
  </div>
{:else}
  <div class="layout">
    <aside>
      <input type="search" placeholder="Bank number or file ID" bind:value={query} />
      <div class="count muted">{rows.length.toLocaleString()} {rows.length === 1 ? "bank" : "banks"}</div>
      <div class="list" bind:clientHeight={viewport} onscroll={(e) => (scrollTop = e.currentTarget.scrollTop)}>
        <div class="spacer" style:height="{rows.length * ROW}px">
          {#each visible as bank, i (bank)}
            <button
              class="row"
              class:active={selected === bank}
              style:top="{(first + i) * ROW}px"
              onclick={() => select(bank)}
            >
              <span class="mono">#{bank}</span>
              <span class="dots">
                {#each languages as l (l.code)}
                  <span
                    class="dot"
                    class:on={l.mfts[bank] >= 0}
                    class:absent={l.files[bank] !== 0 && l.mfts[bank] < 0}
                    title="{l.name}: {l.files[bank] ? `${l.files[bank]}${l.mfts[bank] < 0 ? ' (not installed)' : ''}` : 'none'}"
                  ></span>
                {/each}
              </span>
            </button>
          {/each}
        </div>
      </div>
    </aside>

    <div class="detail">
      <div class="bar">
        <span class="languages">
          {#each languages as l, i (l.code)}
            <button
              class="btn small"
              class:ghost={language !== i}
              class:missing={!installedCount[i]}
              disabled={selected !== null && !l.files[selected]}
              title={installedCount[i] ? `${installedCount[i].toLocaleString()} banks installed` : "Not installed"}
              onclick={() => (language = i)}>{l.name}</button
            >
          {/each}
        </span>
        {#if selected !== null}
          <span class="muted">Bank #{selected}{bankFile ? ` · file ${bankFile}` : ""}</span>
        {/if}
      </div>
      {#if selected === null}
        <div class="message muted">Select a bank.</div>
      {:else if !bankFile}
        <div class="message muted">Bank #{selected} has no {current?.name} version.</div>
      {:else if bankMft < 0}
        <div class="message muted">
          Bank file {bankFile} is not in this archive. The game only downloads the sound banks of its voice language;
          switch the language in the bar to one that is installed.
        </div>
      {:else}
        {#key bankMft}
          <SoundBankViewer mft={bankMft} name={String(bankFile)} />
        {/key}
      {/if}
    </div>
  </div>
{/if}

<style>
  .layout {
    flex: 1;
    display: flex;
    min-height: 0;
  }

  aside {
    width: 200px;
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
    color: inherit;
    font: inherit;
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

  .dots {
    display: flex;
    gap: 4px;
  }

  .dot {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: var(--border);
  }

  .dot.on {
    background: var(--tone-sound);
  }

  .dot.absent {
    background: none;
    box-shadow: inset 0 0 0 1px var(--faint);
  }

  .missing {
    opacity: 0.55;
  }

  .detail {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
  }

  .bar {
    display: flex;
    align-items: center;
    gap: 16px;
    min-height: 40px;
    padding: 4px 12px;
    background: var(--panel-2);
    border-bottom: 1px solid var(--border);
    flex: none;
  }

  .languages {
    display: flex;
    gap: 4px;
  }

  .message {
    padding: 24px;
    text-align: center;
  }

  .error {
    color: var(--accent);
  }
</style>
