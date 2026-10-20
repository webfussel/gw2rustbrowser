<script lang="ts">
  import { api, type TrueTypeInfo } from "../api";
  import { errorMessage } from "../format";

  let { mft }: { mft: number } = $props();

  const family = $derived(`gw2-font-${mft}`);
  let info = $state<TrueTypeInfo | null>(null);
  let error = $state<string | null>(null);
  let ready = $state(false);
  let sample = $state("The quick brown fox jumps over the lazy dog. 0123456789");
  const sizes = [14, 20, 28, 40, 64];

  $effect(() => {
    let stale = false;
    let face: FontFace | null = null;
    ready = false;
    error = null;
    Promise.all([api.truetypeInfo(mft), api.readFile(mft)])
      .then(async ([fontInfo, bytes]) => {
        if (stale) return;
        info = fontInfo;
        face = new FontFace(family, bytes.slice().buffer);
        await face.load();
        if (stale) return;
        document.fonts.add(face);
        ready = true;
      })
      .catch((e) => !stale && (error = `The font could not be loaded: ${errorMessage(e)}`));
    return () => {
      stale = true;
      if (face) document.fonts.delete(face);
    };
  });

  const hex = (c: number) => `U+${c.toString(16).toUpperCase().padStart(4, "0")}`;
  const displayName = $derived(info?.fullName || info?.family || "Name hidden by the font");
</script>

<div class="wrap">
  {#if error}
    <p class="error">{error}</p>
  {:else if !info}
    <p class="muted">Loading…</p>
  {:else}
    <header>
      <h2>{displayName}</h2>
      <p class="muted">
        TrueType · {info.glyphs} glyphs · {info.characters.length} characters{info.subfamily ? ` · ${info.subfamily}` : ""}
      </p>
      {#if info.version}<p class="version muted">{info.version}</p>{/if}
    </header>

    {#if ready}
      <section>
        <input type="text" bind:value={sample} aria-label="Sample text" />
        {#each sizes as size}
          <div class="sample" style:font-family="'{family}'" style:font-size="{size}px">
            <span class="size mono">{size}</span>{sample}
          </div>
        {/each}
      </section>

      <section>
        <h3>Characters</h3>
        <div class="grid" style:font-family="'{family}'">
          {#each info.characters as c (c)}
            <div class="cell" title="{hex(c)}">
              <span class="glyph">{String.fromCodePoint(c)}</span>
              <span class="code mono">{c.toString(16).toUpperCase().padStart(4, "0")}</span>
            </div>
          {/each}
        </div>
      </section>
    {:else}
      <p class="muted">Loading font…</p>
    {/if}
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

  header h2 {
    margin: 0;
    font-size: 18px;
  }

  header p {
    margin: 4px 0 0;
  }

  .version {
    font-size: 12px;
    max-width: 70ch;
  }

  section {
    margin-top: 22px;
  }

  input {
    width: 100%;
    margin-bottom: 12px;
  }

  .sample {
    display: flex;
    align-items: baseline;
    gap: 14px;
    padding: 6px 0;
    border-bottom: 1px solid color-mix(in srgb, var(--border) 60%, transparent);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .size {
    flex: none;
    width: 24px;
    color: var(--faint);
    font-size: 11px;
  }

  h3 {
    margin: 0 0 10px;
    font-size: 11px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    color: var(--muted);
  }

  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(52px, 1fr));
    gap: 4px;
  }

  .cell {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 2px;
    height: 58px;
    border: 1px solid var(--border);
    border-radius: 6px;
    background: var(--panel-2);
  }

  .glyph {
    font-size: 26px;
    line-height: 1;
  }

  .code {
    font-size: 9px;
    color: var(--faint);
  }

  .error {
    color: var(--accent);
  }
</style>
