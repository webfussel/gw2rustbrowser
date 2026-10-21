<script lang="ts">
  import { api, type MaterialInfo, type MaterialShader } from "../api";
  import { errorMessage } from "../format";

  let { mft }: { mft: number } = $props();

  let info = $state.raw<MaterialInfo | null>(null);
  let error = $state<string | null>(null);
  /** Selected shader index; null shows the overview. */
  let selected = $state<number | null>(null);
  let shader = $state.raw<MaterialShader | null>(null);
  let shaderError = $state<string | null>(null);
  let shaderLoading = $state(false);
  let copied = $state(false);

  $effect(() => {
    const id = mft;
    info = null;
    error = null;
    selected = null;
    let stale = false;
    api
      .materialInfo(id)
      .then((m) => !stale && (info = m))
      .catch((e) => !stale && (error = errorMessage(e)));
    return () => (stale = true);
  });

  $effect(() => {
    const index = selected;
    shader = null;
    shaderError = null;
    if (index === null) return;
    let stale = false;
    shaderLoading = true;
    api
      .materialShader(mft, index)
      .then((s) => !stale && (shader = s))
      .catch((e) => !stale && (shaderError = errorMessage(e)))
      .finally(() => !stale && (shaderLoading = false));
    return () => (stale = true);
  });

  /** Effects that use each shader, for the shader list. */
  const usage = $derived.by(() => {
    const uses = new Map<number, string[]>();
    if (!info) return uses;
    const add = (index: number, label: string) => {
      const list = uses.get(index) ?? [];
      if (!list.includes(label)) list.push(label);
      uses.set(index, list);
    };
    for (const t of info.techniques)
      for (const pass of t.passes)
        for (const e of pass) {
          add(e.pixelShader, e.name);
          for (const [variant, vs] of e.vertexShaders) add(vs, `${e.name}/${variant}`);
        }
    return uses;
  });

  const hex = (n: number) => `0x${n.toString(16).padStart(8, "0")}`;

  function registerLabel(u: { kind: string; register: number; registerCount: number }) {
    if (u.kind === "sampler") return `s${u.register}`;
    const first = u.register / 16;
    return u.registerCount > 1 ? `cb0[${first}..${first + u.registerCount - 1}]` : `cb0[${first}]`;
  }

  function codeText(s: MaterialShader) {
    const lines = s.lines.map((l) => {
      const text = "  ".repeat(l.depth) + l.text;
      return l.note ? `${text.padEnd(60)} // ${l.note}` : text;
    });
    return [s.profile ?? "", ...lines].join("\n");
  }

  async function copyCode() {
    if (!shader) return;
    await navigator.clipboard.writeText(codeText(shader));
    copied = true;
    setTimeout(() => (copied = false), 1200);
  }

  /** Splits an instruction into opcode and operands for highlighting. */
  function split(text: string): [string, string] {
    const space = text.indexOf(" ");
    return space < 0 ? [text, ""] : [text.slice(0, space), text.slice(space)];
  }
</script>

{#if error}
  <div class="message error">{error}</div>
{:else if !info}
  <div class="message muted">Loading material…</div>
{:else}
  <div class="layout">
    <aside>
      <button class="row" class:active={selected === null} onclick={() => (selected = null)}>
        <span>Overview</span>
      </button>
      <div class="heading muted">{info.shaders.length} shaders</div>
      <div class="list">
        {#each info.shaders as s, i (i)}
          <button class="row shader" class:active={selected === i} onclick={() => (selected = i)}>
            <span class="line">
              <span class="mono index">#{i}</span>
              <span class="stage" class:pixel={s.stage === "pixel"}>{s.stage}</span>
              <span class="mono muted small">{s.instructions} instr.</span>
            </span>
            <span class="muted small uses">{usage.get(i)?.join(", ") ?? "unused"}</span>
          </button>
        {/each}
      </div>
    </aside>

    <div class="detail">
      {#if selected === null}
        <header>
          <h2>Material</h2>
          <span class="muted">
            BGFX v{info.version} · {info.techniques.length} techniques · {info.shaders.length} shaders · {info.samplers.length}
            samplers
          </span>
        </header>

        {#if info.settings}
          <section>
            <h3>Texture slots</h3>
            {#if info.settings.textures.length}
              <div class="chips">
                {#each info.settings.textures as t, i (i)}
                  <span class="chip"><span class="mono muted">{i}</span> {t}</span>
                {/each}
              </div>
            {:else}
              <p class="muted">No textures.</p>
            {/if}
            <dl>
              <dt>Sort order</dt>
              <dd>{info.settings.sortOrder}{info.settings.sortTriangles ? " · sorts triangles" : ""}</dd>
              <dt>Flags</dt>
              <dd class="mono">{hex(info.settings.flags)}</dd>
              {#if info.settings.debugFlags}
                <dt>Debug flags</dt>
                <dd class="mono">{hex(info.settings.debugFlags)}</dd>
              {/if}
              {#if info.settings.proceduralAnimation}
                <dt>Procedural animation</dt>
                <dd>{info.settings.proceduralAnimation}</dd>
              {/if}
              {#if info.settings.textureArrayRange}
                <dt>Texture array range</dt>
                <dd>{info.settings.textureArrayRange}</dd>
              {/if}
            </dl>
          </section>
        {/if}

        {#each info.techniques as t, ti (ti)}
          <section>
            <h3>Technique {ti + 1} <span class="mono muted small">{hex(t.id)}</span></h3>
            {#each t.passes as pass, pi (pi)}
              {#if t.passes.length > 1}<div class="muted small pass">Pass {pi + 1}</div>{/if}
              <table>
                <thead>
                  <tr><th>Effect</th><th>Render state</th><th>Pixel shader</th><th>Vertex shaders (variant → shader)</th></tr>
                </thead>
                <tbody>
                  {#each pass as e, ei (ei)}
                    <tr>
                      <td class="mono">{e.name}</td>
                      <td title={`${e.renderState}, pass flags ${hex(e.passFlags)}`}>
                        {#if e.state.length}{e.state.join(" · ")}{:else}<span class="muted">default</span>{/if}
                      </td>
                      <td><button class="link mono" onclick={() => (selected = e.pixelShader)}>#{e.pixelShader}</button></td>
                      <td>
                        {#each e.vertexShaders as [variant, vs] (variant)}
                          <span class="variant">
                            <span class="muted mono">{variant}→</span><button class="link mono" onclick={() => (selected = vs)}
                              >#{vs}</button
                            >
                          </span>
                        {/each}
                      </td>
                    </tr>
                  {/each}
                </tbody>
              </table>
            {/each}
          </section>
        {/each}

        {#if info.samplers.length}
          <section>
            <h3>Sampler states</h3>
            <table>
              <tbody>
                {#each info.samplers as s, i (i)}
                  <tr><td class="mono narrow">{i}</td><td>{s}</td></tr>
                {/each}
              </tbody>
            </table>
          </section>
        {/if}
      {:else if shaderError}
        <div class="error">{shaderError}</div>
      {:else if shaderLoading || !shader}
        <div class="muted">Loading shader…</div>
      {:else}
        <header>
          <h2>Shader #{selected}</h2>
          <span class="muted">
            {shader.stage} · {shader.profile ?? "no program"} · bgfx v{shader.bgfxVersion} · constant buffer {shader.constantBufferSize}
            bytes
          </span>
          <button class="btn small ghost copy" onclick={copyCode} disabled={!shader.lines.length}>
            {copied ? "Copied" : "Copy code"}
          </button>
        </header>
        {#if usage.get(selected)}
          <p class="muted">Used by {usage.get(selected)?.join(", ")}</p>
        {/if}

        <div class="grid">
          {#if shader.uniforms.length}
            <section>
              <h3>Uniforms</h3>
              <table>
                <tbody>
                  {#each shader.uniforms as u (u.name)}
                    <tr>
                      <td class="mono">{u.name}</td>
                      <td class="muted">{u.kind}{u.count > 1 ? `[${u.count}]` : ""}</td>
                      <td class="mono muted">{registerLabel(u)}</td>
                    </tr>
                  {/each}
                </tbody>
              </table>
            </section>
          {/if}
          {#each [["Inputs", shader.inputs], ["Outputs", shader.outputs]] as const as [title, signature] (title)}
            {#if signature.length}
              <section>
                <h3>{title}</h3>
                <table>
                  <tbody>
                    {#each signature as s (`${s.semantic}${s.register}`)}
                      <tr>
                        <td class="mono">{s.semantic}</td>
                        <td class="mono muted">{title === "Inputs" ? "v" : "o"}{s.register}.{s.mask}</td>
                        <td class="muted">{s.componentType}</td>
                      </tr>
                    {/each}
                  </tbody>
                </table>
              </section>
            {/if}
          {/each}
          {#if shader.attributes.length || shader.samplers.length}
            <section>
              {#if shader.attributes.length}
                <h3>Vertex attributes</h3>
                <p class="mono">{shader.attributes.join(", ")}</p>
              {/if}
              {#if shader.samplers.length}
                <h3>Texture bindings</h3>
                {#each shader.samplers as s (s)}<p class="mono small">{s}</p>{/each}
              {/if}
            </section>
          {/if}
        </div>

        <section>
          <h3>Disassembly</h3>
          {#if shader.error}
            <p class="error">{shader.error}</p>
          {:else}
            <pre class="code"><span class="profile">{shader.profile}</span>
{#each shader.lines as l, i (i)}{@const [op, rest] = split(l.text)}<span class="ln"
                  ><span class:decl={op.startsWith("dcl_")} class="op">{"  ".repeat(l.depth)}{op}</span>{rest}{#if l.note}<span
                      class="note">  // {l.note}</span
                    >{/if}</span
                >
{/each}</pre>
          {/if}
        </section>
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
    width: 260px;
    flex: none;
    display: flex;
    flex-direction: column;
    background: var(--panel);
    border-right: 1px solid var(--border);
  }

  .heading {
    padding: 10px 12px 4px;
    font-size: 11px;
    text-transform: uppercase;
    letter-spacing: 0.04em;
  }

  .list {
    flex: 1;
    overflow: auto;
  }

  .row {
    width: 100%;
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 2px;
    padding: 7px 12px;
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

  .line {
    display: flex;
    align-items: baseline;
    gap: 8px;
  }

  .index {
    min-width: 28px;
  }

  .stage {
    color: var(--tone-model);
  }

  .stage.pixel {
    color: var(--tone-image);
  }

  .uses {
    max-width: 100%;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .small {
    font-size: 11px;
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

  h3 {
    margin: 0 0 8px;
    font-size: 13px;
    font-weight: 600;
  }

  section {
    margin-bottom: 20px;
  }

  .copy {
    margin-left: auto;
  }

  .chips {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    margin-bottom: 10px;
  }

  .chip {
    padding: 2px 8px;
    border: 1px solid var(--border);
    border-radius: 999px;
    background: var(--panel-2);
  }

  dl {
    display: grid;
    grid-template-columns: max-content 1fr;
    gap: 4px 16px;
    margin: 0;
  }

  dt {
    color: var(--muted);
  }

  dd {
    margin: 0;
  }

  .pass {
    margin: 8px 0 4px;
  }

  table {
    border-collapse: collapse;
    width: 100%;
  }

  th {
    text-align: left;
    font-weight: 500;
    color: var(--muted);
    font-size: 11px;
    padding: 4px 10px 4px 0;
    border-bottom: 1px solid var(--border);
  }

  td {
    padding: 4px 10px 4px 0;
    border-bottom: 1px solid var(--border);
    vertical-align: top;
  }

  .narrow {
    width: 32px;
  }

  .link {
    padding: 0;
    border: 0;
    background: none;
    color: var(--tone-text);
    cursor: pointer;
    font-size: inherit;
  }

  .link:hover {
    text-decoration: underline;
  }

  .variant {
    margin-right: 10px;
    white-space: nowrap;
  }

  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(240px, 1fr));
    gap: 0 24px;
  }

  .grid p {
    margin: 0 0 8px;
  }

  .code {
    margin: 0;
    padding: 12px 14px;
    border: 1px solid var(--border);
    border-radius: var(--radius);
    background: var(--panel);
    font: 12px/1.55 var(--mono);
    overflow-x: auto;
    tab-size: 2;
  }

  .profile {
    color: var(--tone-font);
  }

  .op {
    color: var(--tone-text);
  }

  .op.decl {
    color: var(--muted);
  }

  .note {
    color: var(--tone-map);
  }

  .message {
    padding: 24px;
  }

  .error {
    color: var(--accent);
  }
</style>
