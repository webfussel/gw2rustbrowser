<script lang="ts">
  import { onMount } from "svelte";
  import * as THREE from "three";
  import { OrbitControls } from "three/examples/jsm/controls/OrbitControls.js";
  import { api, type AnimationRef, type Clip, type ModelMesh, type Rig } from "../api";
  import { errorMessage } from "../format";

  let { mft }: { mft: number } = $props();

  let container: HTMLDivElement | undefined = $state();
  let error = $state<string | null>(null);
  let loading = $state(true);
  let stats = $state<{ meshes: number; vertices: number; triangles: number; bones: number } | null>(null);
  let wireframe = $state(false);
  let colorMeshes = $state(true);
  let showGrid = $state(true);
  let showBones = $state(false);
  let useTextures = $state(true);
  // Many diffuse textures use alpha as a mask (e.g. gloss), not as opacity;
  // cutting at alpha would hide those parts. Opt-in for foliage and hair.
  let alphaCutout = $state(false);
  let texturedCount = $state(0);

  // Animation state
  let animations = $state.raw<AnimationRef[]>([]);
  let animationFilter = $state("");
  let selectedAnimation = $state("");
  let playing = $state(false);
  let speed = $state(1);
  let time = $state(0);
  let duration = $state(0);
  let clipLoading = $state(false);
  let clipError = $state<string | null>(null);

  const palette = [0xd9a066, 0x7fb3d5, 0xa3d977, 0xe08f8f, 0xc39bd3, 0xf0d264, 0x76d7c4, 0xf5b7b1];
  const neutral = 0xc8c2b8;

  let renderer: THREE.WebGLRenderer | null = null;
  let scene: THREE.Scene;
  let camera: THREE.PerspectiveCamera;
  let controls: OrbitControls;
  /** Holds everything in the file's space; its matrix converts to Y-up. */
  const root = new THREE.Group();
  let grid: THREE.GridHelper | null = null;
  let skeleton: THREE.Skeleton | null = null;
  let skeletonHelper: THREE.SkeletonHelper | null = null;
  let boneIndex = new Map<string, number>();
  let mixer: THREE.AnimationMixer | null = null;
  let action = $state.raw<THREE.AnimationAction | null>(null);
  const clock = new THREE.Clock();
  let materials: THREE.MeshStandardMaterial[] = [];
  let maps: (THREE.DataTexture | null)[] = [];
  let textures = new Map<number, Promise<THREE.DataTexture | null>>();
  let disposed = false;
  let frame = 0;
  let fitDistance = 1;
  let center = new THREE.Vector3();

  // GW2 stores positions with Z pointing down: (x, y, z) -> (x, -z, -y).
  root.matrixAutoUpdate = false;
  root.matrix.set(1, 0, 0, 0, 0, 0, -1, 0, 0, -1, 0, 0, 0, 0, 0, 1);

  function render() {
    if (renderer) renderer.render(scene, camera);
  }

  function requestRender() {
    if (frame) return;
    frame = requestAnimationFrame(tick);
  }

  function tick() {
    frame = 0;
    if (mixer && playing) {
      mixer.update(clock.getDelta() * speed);
      time = action?.time ?? 0;
      frame = requestAnimationFrame(tick);
    }
    render();
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
    await Promise.all(
      meshes.map(async (m, i) => {
        const diffuse = info[m.materialIndex]?.diffuse;
        if (!diffuse || !m.uvs) return;
        const texture = await loadTexture(diffuse);
        if (disposed || !texture || !materials[i]) return;
        maps[i] = texture;
        texturedCount++;
        updateMaterials();
      }),
    );
  }

  function updateMaterials() {
    for (const [i, m] of materials.entries()) {
      if (!m) continue;
      const map = useTextures ? (maps[i] ?? null) : null;
      const alphaTest = map && alphaCutout ? 0.3 : 0;
      if (m.map !== map || m.alphaTest !== alphaTest) {
        m.map = map;
        m.alphaTest = alphaTest;
        m.needsUpdate = true;
      }
      m.wireframe = wireframe;
      m.color.setHex(map ? 0xffffff : colorMeshes ? palette[i % palette.length] : neutral);
    }
    if (grid) grid.visible = showGrid;
    if (skeletonHelper) skeletonHelper.visible = showBones;
    requestRender();
  }

  function buildSkeleton(rig: Rig): THREE.Skeleton {
    const bones = rig.bones.map((b, i) => {
      const bone = new THREE.Bone();
      // Track names contain ':' which three.js property paths do not allow.
      bone.name = `bone${i}`;
      bone.position.fromArray(b.translation);
      bone.quaternion.fromArray(b.rotation);
      bone.scale.fromArray(b.scale);
      return bone;
    });
    rig.bones.forEach((b, i) => (b.parent !== null ? bones[b.parent] : root).add(bones[i]));
    boneIndex = new Map(rig.bones.map((b, i) => [b.name, i]));
    return new THREE.Skeleton(bones);
  }

  function build(meshes: ModelMesh[], rig: Rig) {
    const skinned = rig.bones.length > 0 && meshes.some((m) => m.skinIndices);
    if (skinned) skeleton = buildSkeleton(rig);
    root.updateMatrixWorld(true);

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
      const material = new THREE.MeshStandardMaterial({
        color: palette[i % palette.length],
        roughness: 0.75,
        metalness: 0.05,
        side: THREE.DoubleSide,
      });
      materials[i] = material;
      maps[i] = null;
      if (skeleton && m.skinIndices && m.skinWeights) {
        geometry.setAttribute("skinIndex", new THREE.Uint16BufferAttribute(m.skinIndices, 4));
        geometry.setAttribute("skinWeight", new THREE.BufferAttribute(m.skinWeights, 4, true));
        const mesh = new THREE.SkinnedMesh(geometry, material);
        // Bounds are those of the rest pose; animated parts may leave them.
        mesh.frustumCulled = false;
        root.add(mesh);
        mesh.bind(skeleton);
      } else {
        root.add(new THREE.Mesh(geometry, material));
      }
      vertices += m.positions.length / 3;
      triangles += m.indices.length / 3;
    });
    stats = { meshes: meshes.length, vertices, triangles, bones: skeleton ? rig.bones.length : 0 };
    if (skeleton) {
      skeletonHelper = new THREE.SkeletonHelper(root);
      skeletonHelper.visible = showBones;
      scene.add(skeletonHelper);
      mixer = new THREE.AnimationMixer(root);
      animations = rig.animations;
    }

    root.updateMatrixWorld(true);
    const box = new THREE.Box3().setFromObject(root);
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

  function toClip(clip: Clip): THREE.AnimationClip {
    const tracks: THREE.KeyframeTrack[] = [];
    for (const t of clip.tracks) {
      const i = boneIndex.get(t.bone);
      if (i === undefined) continue;
      tracks.push(new THREE.QuaternionKeyframeTrack(`bone${i}.quaternion`, t.rotationTimes, t.rotations));
      tracks.push(new THREE.VectorKeyframeTrack(`bone${i}.position`, t.translationTimes, t.translations));
      tracks.push(new THREE.VectorKeyframeTrack(`bone${i}.scale`, t.scaleTimes, t.scales));
    }
    return new THREE.AnimationClip(clip.name, clip.duration, tracks);
  }

  async function selectAnimation(key: string) {
    selectedAnimation = key;
    if (!mixer || !skeleton) return;
    mixer.stopAllAction();
    action = null;
    clipError = null;
    if (!key) {
      restPose();
      return;
    }
    const ref = animations.find((a) => `${a.source}:${a.index}` === key);
    if (!ref) return;
    clipLoading = true;
    try {
      const clip = await api.modelClip(ref.source, ref.index);
      if (disposed || selectedAnimation !== key) return;
      action = mixer.clipAction(toClip(clip));
      action.setLoop(THREE.LoopRepeat, Infinity);
      action.play();
      duration = clip.duration;
      time = 0;
      playing = true;
      clock.getDelta();
      requestRender();
    } catch (e) {
      clipError = errorMessage(e);
    } finally {
      clipLoading = false;
    }
  }

  function restPose() {
    playing = false;
    mixer?.stopAllAction();
    skeleton?.pose();
    time = 0;
    duration = 0;
    requestRender();
  }

  function togglePlay() {
    if (!action) return;
    playing = !playing;
    clock.getDelta();
    requestRender();
  }

  function scrub(value: number) {
    if (!action || !mixer) return;
    playing = false;
    action.time = value;
    mixer.update(0);
    time = value;
    requestRender();
  }

  const filteredAnimations = $derived.by(() => {
    const needle = animationFilter.trim().toLowerCase();
    const list = needle ? animations.filter((a) => a.name.toLowerCase().includes(needle)) : animations;
    const groups = new Map<string, AnimationRef[]>();
    for (const a of list) {
      let g = groups.get(a.group);
      if (!g) groups.set(a.group, (g = []));
      g.push(a);
    }
    return [...groups.entries()];
  });

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
    scene.add(root);

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
    Promise.all([api.loadModel(mft), api.modelRig(mft).catch(() => ({ bones: [], skeletonFile: null, animations: [] }) as Rig)])
      .then(([meshes, rig]) => !stale && build(meshes, rig))
      .catch((e) => !stale && (error = errorMessage(e)))
      .finally(() => !stale && (loading = false));

    return () => {
      stale = true;
      disposed = true;
      playing = false;
      for (const t of textures.values()) t.then((texture) => texture?.dispose());
      resize.disconnect();
      cancelAnimationFrame(frame);
      controls.dispose();
      mixer?.stopAllAction();
      root.traverse((o) => {
        if (o instanceof THREE.Mesh) o.geometry.dispose();
      });
      materials.forEach((m) => m?.dispose());
      skeletonHelper?.dispose();
      grid?.dispose();
      renderer?.dispose();
      renderer?.domElement.remove();
      renderer = null;
    };
  });

  $effect(() => {
    void [wireframe, colorMeshes, showGrid, showBones, useTextures, alphaCutout];
    updateMaterials();
  });
</script>

<div class="bar">
  <button class="btn small ghost" onclick={resetView}>Reset view</button>
  <label><input type="checkbox" bind:checked={useTextures} disabled={texturedCount === 0} /> Textures</label>
  <label title="Hide pixels with low texture alpha (foliage, hair). Off by default: many textures use alpha for other purposes.">
    <input type="checkbox" bind:checked={alphaCutout} disabled={texturedCount === 0 || !useTextures} /> Alpha cutout
  </label>
  <label><input type="checkbox" bind:checked={wireframe} /> Wireframe</label>
  <label><input type="checkbox" bind:checked={colorMeshes} /> Color meshes</label>
  <label><input type="checkbox" bind:checked={showGrid} /> Grid</label>
  {#if stats && stats.bones > 1}
    <label><input type="checkbox" bind:checked={showBones} /> Bones</label>
  {/if}
  {#if stats}
    <span class="mono muted info">
      {stats.meshes} meshes · {stats.vertices.toLocaleString()} vertices · {stats.triangles.toLocaleString()} triangles{stats.bones
        ? ` · ${stats.bones} bones`
        : ""}
    </span>
  {/if}
</div>

{#if animations.length > 0}
  <div class="anim">
    <input type="search" placeholder="Filter {animations.length.toLocaleString()} animations…" bind:value={animationFilter} />
    <select value={selectedAnimation} onchange={(e) => selectAnimation(e.currentTarget.value)}>
      <option value="">Rest pose</option>
      {#each filteredAnimations as [group, list] (group)}
        <optgroup label={group}>
          {#each list as a (`${a.source}:${a.index}`)}
            <option value={`${a.source}:${a.index}`}>{a.name}</option>
          {/each}
        </optgroup>
      {/each}
    </select>
    <button class="btn small" onclick={togglePlay} disabled={!action} aria-label={playing ? "Pause" : "Play"}>
      {playing ? "❚❚" : "▶"}
    </button>
    <input
      class="timeline"
      type="range"
      min="0"
      max={duration || 1}
      step="0.001"
      value={time % (duration || 1)}
      disabled={!action}
      oninput={(e) => scrub(Number(e.currentTarget.value))}
    />
    <span class="mono muted time">{(time % (duration || 1)).toFixed(2)} / {duration.toFixed(2)} s</span>
    <select bind:value={speed} title="Playback speed">
      {#each [0.1, 0.25, 0.5, 1, 1.5, 2] as s}
        <option value={s}>{s}×</option>
      {/each}
    </select>
    {#if clipLoading}<span class="muted">Loading…</span>{/if}
    {#if clipError}<span class="error" title={clipError}>Animation could not be read</span>{/if}
  </div>
{/if}

<div class="stage" bind:this={container}>
  {#if loading}
    <div class="overlay muted">Loading model…</div>
  {:else if error}
    <div class="overlay error">{error}</div>
  {/if}
</div>
<div class="hint muted">Drag to orbit · right-drag to pan · scroll to zoom</div>

<style>
  .bar,
  .anim {
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

  .anim {
    gap: 8px;
    padding: 5px 12px;
  }

  .anim input[type="search"] {
    width: 170px;
  }

  .anim select {
    height: 28px;
    padding: 0 6px;
    border: 1px solid var(--border);
    border-radius: var(--radius);
    background: var(--panel-2);
    color: var(--text);
    font: inherit;
  }

  .anim select:first-of-type {
    max-width: 240px;
  }

  .timeline {
    flex: 1;
    min-width: 120px;
    accent-color: var(--accent);
  }

  .time {
    min-width: 96px;
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
