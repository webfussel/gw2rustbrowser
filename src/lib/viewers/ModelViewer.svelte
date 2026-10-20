<script lang="ts">
  import { onMount } from "svelte";
  import * as THREE from "three";
  import { OrbitControls } from "three/examples/jsm/controls/OrbitControls.js";
  import { api, type ModelMesh } from "../api";
  import { errorMessage } from "../format";

  let { mft }: { mft: number } = $props();

  let container: HTMLDivElement | undefined = $state();
  let error = $state<string | null>(null);
  let loading = $state(true);
  let stats = $state<{ meshes: number; vertices: number; triangles: number } | null>(null);
  let wireframe = $state(false);
  let colorMeshes = $state(true);
  let showGrid = $state(true);
  let useTextures = $state(true);
  let texturedCount = $state(0);

  const palette = [0xd9a066, 0x7fb3d5, 0xa3d977, 0xe08f8f, 0xc39bd3, 0xf0d264, 0x76d7c4, 0xf5b7b1];
  const neutral = 0xc8c2b8;

  let renderer: THREE.WebGLRenderer | null = null;
  let scene: THREE.Scene;
  let camera: THREE.PerspectiveCamera;
  let controls: OrbitControls;
  let group = new THREE.Group();
  let grid: THREE.GridHelper | null = null;
  let materials: THREE.MeshStandardMaterial[] = [];
  /** Diffuse texture per mesh material (same order as `materials`). */
  let maps: (THREE.DataTexture | null)[] = [];
  let textures = new Map<number, Promise<THREE.DataTexture | null>>();
  let disposed = false;
  let frame = 0;
  let fitDistance = 1;
  let center = new THREE.Vector3();

  function render() {
    if (renderer) renderer.render(scene, camera);
  }

  function requestRender() {
    if (frame) return;
    frame = requestAnimationFrame(() => {
      frame = 0;
      render();
    });
  }

  function loadTexture(fileId: number): Promise<THREE.DataTexture | null> {
    let texture = textures.get(fileId);
    if (!texture) {
      texture = api
        .loadTexture(fileId)
        .then((image) => {
          const t = new THREE.DataTexture(image.pixels, image.width, image.height, THREE.RGBAFormat);
          t.colorSpace = THREE.SRGBColorSpace;
          t.wrapS = t.wrapT = THREE.RepeatWrapping;
          t.generateMipmaps = true;
          t.minFilter = THREE.LinearMipmapLinearFilter;
          t.magFilter = THREE.LinearFilter;
          t.anisotropy = renderer?.capabilities.getMaxAnisotropy() ?? 1;
          t.needsUpdate = true;
          return t;
        })
        .catch(() => null);
      textures.set(fileId, texture);
    }
    return texture;
  }

  async function applyTextures(meshes: ModelMesh[]) {
    let info;
    try {
      info = await api.modelMaterials(mft);
    } catch {
      return; // Untextured preview still works.
    }
    const loads = meshes.map(async (m, i) => {
      const diffuse = info[m.materialIndex]?.diffuse;
      if (!diffuse || !m.uvs) return;
      const texture = await loadTexture(diffuse);
      if (disposed || !texture || !materials[i]) return;
      maps[i] = texture;
      texturedCount++;
      updateMaterials();
    });
    await Promise.all(loads);
  }

  function updateMaterials() {
    for (const [i, m] of materials.entries()) {
      if (!m) continue;
      const map = useTextures ? (maps[i] ?? null) : null;
      if (m.map !== map) {
        m.map = map;
        m.needsUpdate = true;
      }
      m.alphaTest = map ? 0.3 : 0;
      m.wireframe = wireframe;
      m.color.setHex(map ? 0xffffff : colorMeshes ? palette[i % palette.length] : neutral);
    }
    if (grid) grid.visible = showGrid;
    requestRender();
  }

  function build(meshes: ModelMesh[]) {
    const box = new THREE.Box3();
    let vertices = 0;
    let triangles = 0;
    meshes.forEach((m, i) => {
      if (m.indices.length === 0) return;
      const geometry = new THREE.BufferGeometry();
      geometry.setAttribute("position", new THREE.BufferAttribute(m.positions, 3));
      if (m.normals) geometry.setAttribute("normal", new THREE.BufferAttribute(m.normals, 3));
      if (m.uvs) geometry.setAttribute("uv", new THREE.BufferAttribute(m.uvs, 2));
      geometry.setIndex(new THREE.BufferAttribute(m.indices, 1));
      if (!m.normals) geometry.computeVertexNormals();
      geometry.computeBoundingBox();
      box.union(geometry.boundingBox!);
      const material = new THREE.MeshStandardMaterial({
        color: palette[i % palette.length],
        roughness: 0.75,
        metalness: 0.05,
        side: THREE.DoubleSide,
      });
      materials[i] = material;
      maps[i] = null;
      group.add(new THREE.Mesh(geometry, material));
      vertices += m.positions.length / 3;
      triangles += m.indices.length / 3;
    });
    stats = { meshes: meshes.length, vertices, triangles };
    if (box.isEmpty()) return;

    const size = box.getSize(new THREE.Vector3());
    box.getCenter(center);
    const radius = Math.max(size.length() / 2, 1e-3);
    fitDistance = radius / Math.sin((camera.fov * Math.PI) / 360);
    camera.near = radius / 100;
    camera.far = radius * 100;
    camera.updateProjectionMatrix();

    const extent = Math.max(size.x, size.z) * 1.6;
    grid = new THREE.GridHelper(extent, 16, 0x666666, 0x3a3a3a);
    grid.position.set(center.x, box.min.y, center.z);
    grid.visible = showGrid;
    scene.add(grid);
    resetView();
    applyTextures(meshes);
  }

  function resetView() {
    const direction = new THREE.Vector3(1, 0.6, 1.4).normalize();
    camera.position.copy(center).addScaledVector(direction, fitDistance * 1.05);
    controls.target.copy(center);
    controls.update();
    requestRender();
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
    const key = new THREE.DirectionalLight(0xffffff, 1.8);
    key.position.set(1, 2, 1.5);
    camera.add(key);
    scene.add(camera);
    scene.add(group);

    controls = new OrbitControls(camera, renderer.domElement);
    controls.enableDamping = false;
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
      .loadModel(mft)
      .then((meshes) => !stale && build(meshes))
      .catch((e) => !stale && (error = errorMessage(e)))
      .finally(() => !stale && (loading = false));

    return () => {
      stale = true;
      disposed = true;
      for (const t of textures.values()) t.then((texture) => texture?.dispose());
      resize.disconnect();
      cancelAnimationFrame(frame);
      controls.dispose();
      group.traverse((o) => {
        if (o instanceof THREE.Mesh) o.geometry.dispose();
      });
      materials.forEach((m) => m?.dispose());
      grid?.dispose();
      renderer?.dispose();
      renderer?.domElement.remove();
      renderer = null;
    };
  });

  $effect(() => {
    void [wireframe, colorMeshes, showGrid, useTextures];
    updateMaterials();
  });
</script>

<div class="bar">
  <button class="btn small ghost" onclick={resetView}>Reset view</button>
  <label><input type="checkbox" bind:checked={useTextures} disabled={texturedCount === 0} /> Textures</label>
  <label><input type="checkbox" bind:checked={wireframe} /> Wireframe</label>
  <label><input type="checkbox" bind:checked={colorMeshes} /> Color meshes</label>
  <label><input type="checkbox" bind:checked={showGrid} /> Grid</label>
  {#if stats}
    <span class="mono muted info">
      {stats.meshes} meshes · {stats.vertices.toLocaleString()} vertices · {stats.triangles.toLocaleString()} triangles
    </span>
  {/if}
</div>
<div class="stage" bind:this={container}>
  {#if loading}
    <div class="overlay muted">Loading model…</div>
  {:else if error}
    <div class="overlay error">{error}</div>
  {/if}
</div>
<div class="hint muted">Drag to orbit · right-drag to pan · scroll to zoom</div>

<style>
  .bar {
    display: flex;
    align-items: center;
    gap: 14px;
    height: 38px;
    padding: 0 12px;
    background: var(--panel);
    border-bottom: 1px solid var(--border);
    flex: none;
  }

  .info {
    margin-left: auto;
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
