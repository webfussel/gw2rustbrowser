<script lang="ts">
  import { onMount } from "svelte";
  import * as THREE from "three";
  import { OrbitControls } from "three/examples/jsm/controls/OrbitControls.js";
  import { api, type CollisionData } from "../api";
  import { errorMessage } from "../format";
  import { showFile } from "../state.svelte";

  let { mft }: { mft: number } = $props();

  const palette = [0xe0a458, 0x7fb3d5, 0xa3d977, 0xe08f8f, 0xc39bd3, 0xf0d264, 0x76d7c4, 0xf5b7b1];

  let container: HTMLDivElement | undefined = $state();
  let data = $state.raw<CollisionData | null>(null);
  let error = $state<string | null>(null);
  let loading = $state(true);
  let selectedState = $state(-1);
  let wireframe = $state(false);
  let showModel = $state(true);
  let modelLoaded = $state(false);

  let renderer: THREE.WebGLRenderer | null = null;
  let scene: THREE.Scene;
  let camera: THREE.PerspectiveCamera;
  let controls: OrbitControls;
  /** Holds everything in the model's own space (z down); converts to Y-up. */
  const root = new THREE.Group();
  root.matrixAutoUpdate = false;
  root.matrix.set(1, 0, 0, 0, 0, 0, -1, 0, 0, -1, 0, 0, 0, 0, 0, 1);
  const collisionGroup = new THREE.Group();
  const modelGroup = new THREE.Group();
  root.add(modelGroup, collisionGroup);
  let meshes: THREE.Mesh[] = [];
  let blockers: THREE.LineLoop[] = [];
  const materials: THREE.Material[] = [];
  let frame = 0;

  function render() {
    renderer?.render(scene, camera);
  }

  function requestRender() {
    if (frame) return;
    frame = requestAnimationFrame(() => {
      frame = 0;
      render();
    });
  }

  function build(d: CollisionData) {
    d.meshes.forEach((m, i) => {
      const geometry = new THREE.BufferGeometry();
      geometry.setAttribute("position", new THREE.BufferAttribute(new Float32Array(m.vertices), 3));
      geometry.setIndex(new THREE.BufferAttribute(new Uint16Array(m.indices), 1));
      geometry.computeVertexNormals();
      const material = new THREE.MeshStandardMaterial({
        color: palette[i % palette.length],
        roughness: 0.8,
        side: THREE.DoubleSide,
        transparent: true,
        opacity: 0.75,
        polygonOffset: true,
        polygonOffsetFactor: -1,
      });
      materials.push(material);
      const mesh = new THREE.Mesh(geometry, material);
      meshes.push(mesh);
      collisionGroup.add(mesh);
      const edges = new THREE.LineSegments(
        new THREE.EdgesGeometry(geometry, 30),
        new THREE.LineBasicMaterial({ color: 0x1b1d22, transparent: true, opacity: 0.6 }),
      );
      mesh.add(edges);
    });
    d.blockers.forEach((b) => {
      const geometry = new THREE.BufferGeometry();
      geometry.setAttribute("position", new THREE.BufferAttribute(new Float32Array(b), 3));
      const line = new THREE.LineLoop(geometry, new THREE.LineBasicMaterial({ color: 0xff5544 }));
      blockers.push(line);
      collisionGroup.add(line);
    });
    root.updateMatrixWorld(true);
    fit();
  }

  function fit() {
    const box = new THREE.Box3().setFromObject(collisionGroup.children.length ? collisionGroup : modelGroup);
    if (box.isEmpty()) return;
    const center = box.getCenter(new THREE.Vector3());
    const radius = box.getSize(new THREE.Vector3()).length() / 2 || 1;
    const distance = radius / Math.sin((camera.fov * Math.PI) / 360);
    camera.near = distance / 200;
    camera.far = distance * 20;
    camera.position.copy(center).add(new THREE.Vector3(1, 0.7, 1).normalize().multiplyScalar(distance));
    camera.updateProjectionMatrix();
    controls.target.copy(center);
    controls.update();
    requestRender();
  }

  async function loadModel(d: CollisionData) {
    if (!d.model || d.model.fileType === "MODL skeleton") return;
    try {
      const parts = await api.loadModel(d.model.mft);
      const material = new THREE.MeshStandardMaterial({
        color: 0xc8c2b8,
        roughness: 0.9,
        side: THREE.DoubleSide,
        transparent: true,
        opacity: 0.35,
        depthWrite: false,
      });
      materials.push(material);
      for (const p of parts) {
        if (!p.indices.length) continue;
        const geometry = new THREE.BufferGeometry();
        geometry.setAttribute("position", new THREE.BufferAttribute(p.positions, 3));
        geometry.setIndex(new THREE.BufferAttribute(p.indices, 1));
        geometry.computeVertexNormals();
        modelGroup.add(new THREE.Mesh(geometry, material));
      }
      modelLoaded = modelGroup.children.length > 0;
      if (!d.meshes.length && !d.blockers.length) fit();
      requestRender();
    } catch {
      // The collision still shows without its model.
    }
  }

  onMount(() => {
    try {
      renderer = new THREE.WebGLRenderer({ antialias: true, alpha: true });
    } catch (e) {
      error = `WebGL is not available: ${errorMessage(e)}`;
      loading = false;
      return;
    }
    renderer.setPixelRatio(window.devicePixelRatio);
    container!.appendChild(renderer.domElement);
    scene = new THREE.Scene();
    camera = new THREE.PerspectiveCamera(40, 1, 0.1, 1000);
    scene.add(new THREE.HemisphereLight(0xffffff, 0x404048, 1.6));
    const key = new THREE.DirectionalLight(0xffffff, 1.6);
    key.position.set(1, 2, 1.5);
    camera.add(key);
    scene.add(camera, root);
    controls = new OrbitControls(camera, renderer.domElement);
    controls.addEventListener("change", requestRender);
    const resize = new ResizeObserver(() => {
      const { clientWidth: w, clientHeight: h } = container!;
      if (!w || !h) return;
      renderer!.setSize(w, h);
      camera.aspect = w / h;
      camera.updateProjectionMatrix();
      requestRender();
    });
    resize.observe(container!);

    let stale = false;
    api
      .collisionInfo(mft)
      .then((d) => {
        if (stale) return;
        data = d;
        build(d);
        loadModel(d);
      })
      .catch((e) => !stale && (error = errorMessage(e)))
      .finally(() => !stale && (loading = false));

    return () => {
      stale = true;
      resize.disconnect();
      cancelAnimationFrame(frame);
      controls.dispose();
      root.traverse((o) => {
        if (o instanceof THREE.Mesh || o instanceof THREE.LineSegments || o instanceof THREE.Line) {
          o.geometry.dispose();
          if (!Array.isArray(o.material)) o.material.dispose();
        }
      });
      materials.forEach((m) => m.dispose());
      renderer?.dispose();
      renderer?.domElement.remove();
      renderer = null;
    };
  });

  // Animation state: show only the meshes and blockers it uses.
  $effect(() => {
    const animation = selectedState >= 0 ? data?.animations[selectedState] : null;
    meshes.forEach((m, i) => (m.visible = !animation || animation.meshes.includes(i)));
    blockers.forEach((b, i) => (b.visible = !animation || animation.blockers.includes(i)));
    for (const m of materials) if (m instanceof THREE.MeshStandardMaterial && m.opacity > 0.5) m.wireframe = wireframe;
    modelGroup.visible = showModel;
    requestRender();
  });

  const triangles = $derived(data?.meshes.reduce((n, m) => n + m.indices.length / 3, 0) ?? 0);
  const empty = $derived(data !== null && !data.meshes.length && !data.blockers.length);
</script>

<div class="bar">
  {#if data}
    <span class="muted">
      {data.meshes.length} meshes · {triangles.toLocaleString()} triangles{data.blockers.length
        ? ` · ${data.blockers.length} blockers`
        : ""}
    </span>
    {#if data.animations.length > 1}
      <select bind:value={selectedState} title="Collision state (e.g. a door open or closed)">
        <option value={-1}>All states</option>
        {#each data.animations as a, i (i)}
          <option value={i}>{a.name} ({a.meshes.length} meshes)</option>
        {/each}
      </select>
    {/if}
    <label><input type="checkbox" bind:checked={wireframe} /> Wireframe</label>
    {#if data.model}
      <label title="The model this collision belongs to (from the collision manifest)">
        <input type="checkbox" bind:checked={showModel} disabled={!modelLoaded} /> Model
      </label>
      <button class="link mono" onclick={() => showFile(data!.model!.id)}>{data.model.id}</button>
    {/if}
    <button class="btn small ghost" onclick={fit}>Reset view</button>
  {/if}
</div>
<div class="stage" bind:this={container}>
  {#if loading}
    <div class="overlay muted">Loading collision…</div>
  {:else if error}
    <div class="overlay error">{error}</div>
  {:else if empty && !modelLoaded}
    <div class="overlay muted">This collision file is empty: it has no meshes or blockers.</div>
  {/if}
</div>
<div class="hint muted">
  Collision in the model's own units (Havok metres × 32). Coloured: collision meshes; red lines: blockers; grey: the
  model. Drag to orbit · right-drag to pan · scroll to zoom
</div>

<style>
  .bar {
    display: flex;
    align-items: center;
    gap: 14px;
    min-height: 38px;
    padding: 0 12px;
    background: var(--panel);
    border-bottom: 1px solid var(--border);
    flex: none;
    flex-wrap: wrap;
  }

  label {
    display: flex;
    align-items: center;
    gap: 6px;
  }

  select {
    height: 26px;
    padding: 0 6px;
    border: 1px solid var(--border);
    border-radius: var(--radius);
    background: var(--panel-2);
    color: var(--text);
    font: inherit;
  }

  .link {
    padding: 0;
    border: 0;
    background: none;
    color: var(--tone-text);
    cursor: pointer;
    font-size: inherit;
  }

  .stage {
    flex: 1;
    position: relative;
    min-height: 0;
    overflow: hidden;
    background: radial-gradient(ellipse at center, #2a2e36 0%, #15171b 75%);
  }

  .stage :global(canvas) {
    display: block;
  }

  .overlay {
    position: absolute;
    inset: 0;
    display: grid;
    place-items: center;
    padding: 24px;
    text-align: center;
  }

  .error {
    color: var(--accent);
  }

  .hint {
    padding: 4px 12px;
    font-size: 11px;
    background: var(--panel);
    border-top: 1px solid var(--border);
  }
</style>
