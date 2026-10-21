<script lang="ts">
  import { onMount } from "svelte";
  import * as THREE from "three";
  import { MapControls } from "three/examples/jsm/controls/MapControls.js";
  import { api, type MapData, type PropMesh } from "../api";
  import { errorMessage } from "../format";

  let { mft }: { mft: number } = $props();

  /** World z that marks a hole in the terrain. */
  const HOLE = 10000;
  const MAX_LOADS = 4;

  let container: HTMLDivElement | undefined = $state();
  let error = $state<string | null>(null);
  let loading = $state(true);
  let header = $state.raw<Omit<MapData, "heights" | "propModels" | "props"> | null>(null);
  let showProps = $state(true);
  let showMarkers = $state(true);
  let showWater = $state(true);
  let useTextures = $state(true);
  let alphaCutout = $state(false);
  let wireframe = $state(false);
  let drawDistance = $state(8000);
  /** Ignores the draw distance: every prop of the map is loaded and drawn. */
  let showAll = $state(false);
  let showInfo = $state(false);
  let drawn = $state({ props: 0, triangles: 0, models: 0 });
  let pending = $state(0);
  let failed = $state(0);
  let picked = $state<{ model: number; position: [number, number, number]; scale: number } | null>(null);

  let renderer: THREE.WebGLRenderer | null = null;
  let scene: THREE.Scene;
  let camera: THREE.PerspectiveCamera;
  let controls: MapControls;
  /** Holds everything in world space (z down); its matrix converts to Y-up. */
  const root = new THREE.Group();
  root.matrixAutoUpdate = false;
  root.matrix.set(1, 0, 0, 0, 0, 0, -1, 0, 0, -1, 0, 0, 0, 0, 0, 1);
  const propRoot = new THREE.Group();
  root.add(propRoot);
  let terrain: THREE.Mesh | null = null;
  let terrainMaterial: THREE.MeshStandardMaterial | null = null;
  let water: THREE.Group | null = null;
  let markers: THREE.Points | null = null;
  let disposed = false;
  let frame = 0;
  let home = { target: new THREE.Vector3(), position: new THREE.Vector3() };

  /** All props of one model, drawn as one instanced mesh per model mesh. */
  interface PropGroup {
    model: number;
    /** World x/y of each placement, for the distance test. */
    xy: Float32Array;
    /** World-space instance matrices, 16 per placement. */
    matrices: Float32Array;
    state: "idle" | "queued" | "loading" | "ready" | "failed";
    nearest: number;
    meshes: { mesh: THREE.InstancedMesh; diffuse: number | null }[];
    attribute: THREE.InstancedBufferAttribute | null;
    triangles: number;
  }
  let groups: PropGroup[] = [];
  const plainMaterial = new THREE.MeshStandardMaterial({ color: 0xbdb5a6, roughness: 0.85, side: THREE.DoubleSide });
  const textured = new Map<number, Promise<THREE.MeshStandardMaterial | null>>();
  const texturedReady = new Map<number, THREE.MeshStandardMaterial>();
  let active = 0;

  function render() {
    if (renderer) renderer.render(scene, camera);
  }

  function requestRender() {
    if (frame || !renderer) return;
    frame = requestAnimationFrame(() => {
      frame = 0;
      render();
    });
  }

  /** Orbit target in world coordinates. */
  function focus(): [number, number] {
    return [controls.target.x, -controls.target.z];
  }

  function buildTerrain(map: MapData) {
    const { columns, rows, heights, rect } = map;
    if (!columns || !rows) return;
    const [x0, y0, x1, y1] = rect;
    const sx = (x1 - x0) / (columns - 1);
    const sy = (y1 - y0) / (rows - 1);
    const count = columns * rows;
    const positions = new Float32Array(count * 3);
    const normals = new Float32Array(count * 3);
    const colors = new Float32Array(count * 3);
    const valid = (h: number) => h !== HOLE;

    // Colour by height between the 2nd and 98th percentile, so a few deep
    // pits or peaks do not flatten the ramp.
    const sorted = heights.filter(valid).sort();
    const lo = sorted[Math.floor(sorted.length * 0.02)] ?? 0;
    const hi = sorted[Math.floor(sorted.length * 0.98)] ?? 1;
    const fallback = sorted[sorted.length >> 1] ?? 0;
    const sea = map.water?.plane ? map.water.planeZ : null;
    const ramp: [number, [number, number, number]][] = [
      [0, [0.27, 0.36, 0.2]],
      [0.35, [0.36, 0.44, 0.24]],
      [0.65, [0.47, 0.45, 0.33]],
      [0.85, [0.52, 0.48, 0.42]],
      [1, [0.88, 0.89, 0.9]],
    ];
    const rock: [number, number, number] = [0.42, 0.4, 0.37];
    const sand: [number, number, number] = [0.6, 0.55, 0.42];
    const at = (x: number, y: number) => {
      const h = heights[Math.min(rows - 1, Math.max(0, y)) * columns + Math.min(columns - 1, Math.max(0, x))];
      return valid(h) ? h : null;
    };

    for (let y = 0; y < rows; y++) {
      for (let x = 0; x < columns; x++) {
        const i = y * columns + x;
        const h = heights[i];
        const z = valid(h) ? h : fallback;
        positions.set([x0 + x * sx, y0 + y * sy, z], i * 3);
        const fx = ((at(x + 1, y) ?? z) - (at(x - 1, y) ?? z)) / (2 * sx);
        const fy = ((at(x, y + 1) ?? z) - (at(x, y - 1) ?? z)) / (2 * sy);
        // z points down, so the upward normal is (dz/dx, dz/dy, -1).
        const length = Math.hypot(fx, fy, 1);
        normals.set([fx / length, fy / length, -1 / length], i * 3);

        // Height above the low end of the ramp (z points down).
        const t = Math.min(1, Math.max(0, (hi - z) / (hi - lo || 1)));
        let k = 1;
        while (k < ramp.length - 1 && ramp[k][0] < t) k++;
        const [ta, ca] = ramp[k - 1];
        const [tb, cb] = ramp[k];
        const f = (t - ta) / (tb - ta);
        let c = ca.map((v, j) => v + (cb[j] - v) * f);
        const steep = Math.min(1, Math.max(0, (Math.hypot(fx, fy) - 0.6) / 0.6));
        c = c.map((v, j) => v + (rock[j] - v) * steep);
        if (sea !== null && z > sea - 20) c = sand.map((v, j) => v * (z > sea ? 0.75 : 1));
        colors.set(c, i * 3);
      }
    }

    const indices: number[] = [];
    for (let y = 0; y < rows - 1; y++) {
      for (let x = 0; x < columns - 1; x++) {
        const a = y * columns + x;
        const b = a + 1;
        const c = a + columns;
        const d = c + 1;
        if (!valid(heights[a]) || !valid(heights[b]) || !valid(heights[c]) || !valid(heights[d])) continue;
        // Wound so the faces look up (towards negative z).
        indices.push(a, c, b, b, c, d);
      }
    }
    const geometry = new THREE.BufferGeometry();
    geometry.setAttribute("position", new THREE.BufferAttribute(positions, 3));
    geometry.setAttribute("normal", new THREE.BufferAttribute(normals, 3));
    geometry.setAttribute("color", new THREE.BufferAttribute(colors, 3));
    geometry.setIndex(new THREE.BufferAttribute(new Uint32Array(indices), 1));
    terrainMaterial = new THREE.MeshStandardMaterial({ vertexColors: true, roughness: 0.95, side: THREE.DoubleSide });
    terrain = new THREE.Mesh(geometry, terrainMaterial);
    root.add(terrain);
  }

  function buildWater(map: MapData) {
    if (!map.water) return;
    water = new THREE.Group();
    const material = new THREE.MeshStandardMaterial({
      color: 0x2d6f96,
      roughness: 0.25,
      transparent: true,
      opacity: 0.55,
      depthWrite: false,
      side: THREE.DoubleSide,
    });
    const [x0, y0, x1, y1] = map.rect;
    if (map.water.plane) {
      const plane = new THREE.Mesh(new THREE.PlaneGeometry(x1 - x0, y1 - y0), material);
      plane.position.set((x0 + x1) / 2, (y0 + y1) / 2, map.water.planeZ);
      water.add(plane);
    }
    for (const surface of map.water.surfaces) {
      const shape = new THREE.Shape(surface.outline.map(([x, y]) => new THREE.Vector2(x, y)));
      const mesh = new THREE.Mesh(new THREE.ShapeGeometry(shape), material);
      mesh.position.z = surface.z;
      water.add(mesh);
    }
    root.add(water);
  }

  function buildProps(map: MapData) {
    const count = map.propCount;
    const byModel: number[][] = map.models.map(() => []);
    for (let i = 0; i < count; i++) byModel[map.propModels[i * 8]].push(i);

    const markerPositions = new Float32Array(count * 3);
    const matrix = new THREE.Matrix4();
    const quaternion = new THREE.Quaternion();
    const euler = new THREE.Euler();
    const position = new THREE.Vector3();
    const scale = new THREE.Vector3();
    groups = byModel.map((list, model) => {
      const xy = new Float32Array(list.length * 2);
      const matrices = new Float32Array(list.length * 16);
      list.forEach((p, j) => {
        const v = map.props.subarray(p * 8 + 1, p * 8 + 8);
        markerPositions.set([v[0], v[1], v[2]], p * 3);
        xy.set([v[0], v[1]], j * 2);
        // Rotations follow Direct3D's yaw-pitch-roll: z first, then x, then
        // y, each with the opposite sign of a right-handed rotation.
        euler.set(-v[3], -v[4], -v[5], "YXZ");
        quaternion.setFromEuler(euler);
        matrix.compose(position.set(v[0], v[1], v[2]), quaternion, scale.setScalar(v[6]));
        matrix.toArray(matrices, j * 16);
      });
      return { model: map.models[model], xy, matrices, state: "idle", nearest: Infinity, meshes: [], attribute: null, triangles: 0 };
    });

    const geometry = new THREE.BufferGeometry();
    geometry.setAttribute("position", new THREE.BufferAttribute(markerPositions, 3));
    markers = new THREE.Points(
      geometry,
      new THREE.PointsMaterial({ color: 0xff8a3d, size: 2, sizeAttenuation: false, transparent: true, opacity: 0.8 }),
    );
    root.add(markers);
  }

  function materialFor(diffuse: number | null): THREE.Material {
    if (!useTextures || diffuse === null) return plainMaterial;
    const ready = texturedReady.get(diffuse);
    if (ready) return ready;
    if (!textured.has(diffuse)) {
      textured.set(
        diffuse,
        api
          .loadTexture(diffuse)
          .then((image) => {
            if (disposed) return null;
            const t = new THREE.DataTexture(image.pixels, image.width, image.height, THREE.RGBAFormat);
            t.colorSpace = THREE.SRGBColorSpace;
            t.wrapS = t.wrapT = THREE.RepeatWrapping;
            t.generateMipmaps = true;
            t.minFilter = THREE.LinearMipmapLinearFilter;
            t.magFilter = THREE.LinearFilter;
            t.anisotropy = renderer?.capabilities.getMaxAnisotropy() ?? 1;
            t.needsUpdate = true;
            const m = new THREE.MeshStandardMaterial({
              map: t,
              roughness: 0.85,
              side: THREE.DoubleSide,
              alphaTest: alphaCutout ? 0.3 : 0,
            });
            texturedReady.set(diffuse, m);
            applyMaterials();
            return m;
          })
          .catch(() => null),
      );
    }
    return plainMaterial;
  }

  function applyMaterials() {
    for (const g of groups) for (const m of g.meshes) m.mesh.material = materialFor(m.diffuse);
    for (const m of texturedReady.values()) {
      const alphaTest = alphaCutout ? 0.3 : 0;
      if (m.alphaTest !== alphaTest) {
        m.alphaTest = alphaTest;
        m.needsUpdate = true;
      }
    }
    requestRender();
  }

  function createGroupMeshes(group: PropGroup, meshes: PropMesh[]) {
    const n = group.xy.length / 2;
    group.attribute = new THREE.InstancedBufferAttribute(new Float32Array(n * 16), 16);
    group.attribute.setUsage(THREE.DynamicDrawUsage);
    for (const m of meshes) {
      if (m.indices.length === 0) continue;
      const geometry = new THREE.BufferGeometry();
      geometry.setAttribute("position", new THREE.BufferAttribute(m.positions, 3));
      if (m.normals) geometry.setAttribute("normal", new THREE.BufferAttribute(m.normals, 3));
      if (m.uvs) geometry.setAttribute("uv", new THREE.BufferAttribute(m.uvs, 2));
      geometry.setIndex(new THREE.BufferAttribute(m.indices, 1));
      if (!m.normals) geometry.computeVertexNormals();
      const mesh = new THREE.InstancedMesh(geometry, materialFor(m.diffuse), n);
      mesh.instanceMatrix = group.attribute;
      mesh.count = 0;
      // Instances change with the view; their bounds are not tracked.
      mesh.frustumCulled = false;
      propRoot.add(mesh);
      group.meshes.push({ mesh, diffuse: m.diffuse });
      group.triangles += m.indices.length / 3;
    }
  }

  function pump() {
    while (active < MAX_LOADS) {
      let next: PropGroup | null = null;
      for (const g of groups) if (g.state === "queued" && (!next || g.nearest < next.nearest)) next = g;
      if (!next) break;
      const group = next;
      group.state = "loading";
      active++;
      api
        .loadPropModel(group.model)
        .then((meshes) => {
          if (disposed) return;
          createGroupMeshes(group, meshes);
          group.state = "ready";
        })
        .catch(() => {
          group.state = "failed";
          failed++;
        })
        .finally(() => {
          active--;
          if (disposed) return;
          updateProps();
          pump();
        });
    }
  }

  /** Shows the props within the draw distance of the focus point and queues their models. */
  function updateProps() {
    if (!groups.length) return;
    const [fx, fy] = focus();
    const limit = showAll ? Infinity : drawDistance * drawDistance;
    let props = 0;
    let triangles = 0;
    let models = 0;
    let waiting = 0;
    for (const g of groups) {
      let visible = 0;
      let nearest = Infinity;
      const n = g.xy.length / 2;
      for (let i = 0; i < n; i++) {
        const dx = g.xy[i * 2] - fx;
        const dy = g.xy[i * 2 + 1] - fy;
        const d = dx * dx + dy * dy;
        if (d > limit) continue;
        nearest = Math.min(nearest, d);
        if (g.state === "ready" && showProps) g.attribute!.array.set(g.matrices.subarray(i * 16, i * 16 + 16), visible * 16);
        visible++;
      }
      g.nearest = nearest;
      if (visible > 0 && showProps && g.state === "idle") g.state = "queued";
      if (g.state === "queued" && (visible === 0 || !showProps)) g.state = "idle";
      if (g.state === "queued" || g.state === "loading") waiting++;
      if (g.state !== "ready") continue;
      const count = showProps ? visible : 0;
      for (const m of g.meshes) m.mesh.count = count;
      if (count) {
        g.attribute!.needsUpdate = true;
        props += count;
        triangles += count * g.triangles;
        models++;
      }
    }
    drawn = { props, triangles, models };
    pending = waiting;
    pump();
    requestRender();
  }

  let updateTimer = 0;
  function scheduleUpdate() {
    if (updateTimer) return;
    updateTimer = window.setTimeout(() => {
      updateTimer = 0;
      updateProps();
    }, 150);
  }

  function frameMap(map: MapData) {
    const [x0, y0, x1, y1] = map.rect;
    const [zLow, zHigh] = map.zRange ?? [0, 0];
    const size = Math.max(x1 - x0, y1 - y0);
    // World (x, y, z) is (x, -z, -y) in the Y-up view.
    const target = new THREE.Vector3((x0 + x1) / 2, -(zLow + zHigh) / 2, -(y0 + y1) / 2);
    camera.near = Math.max(1, size / 20000);
    camera.far = size * 6;
    camera.updateProjectionMatrix();
    home = { target, position: target.clone().add(new THREE.Vector3(0, size * 0.55, size * 0.55)) };
    scene.fog = new THREE.Fog(0x9fb4c8, size * 0.9, size * 3);
    controls.maxDistance = size * 2.5;
    resetView();
  }

  function resetView() {
    controls.target.copy(home.target);
    camera.position.copy(home.position);
    controls.update();
    scheduleUpdate();
  }

  const raycaster = new THREE.Raycaster();
  function pick(event: MouseEvent): THREE.Intersection | null {
    const rect = renderer!.domElement.getBoundingClientRect();
    const pointer = new THREE.Vector2(
      ((event.clientX - rect.left) / rect.width) * 2 - 1,
      -((event.clientY - rect.top) / rect.height) * 2 + 1,
    );
    raycaster.setFromCamera(pointer, camera);
    const targets: THREE.Object3D[] = [];
    if (terrain) targets.push(terrain);
    for (const g of groups) for (const m of g.meshes) if (m.mesh.count > 0) targets.push(m.mesh);
    return raycaster.intersectObjects(targets, false)[0] ?? null;
  }

  function onClick(event: MouseEvent) {
    const hit = pick(event);
    const group = hit && groups.find((g) => g.meshes.some((m) => m.mesh === hit.object));
    if (!hit || !group || hit.instanceId === undefined) {
      picked = null;
      return;
    }
    // Instance slots are filled in placement order, skipping props out of range.
    const m = new THREE.Matrix4().fromArray(group.attribute!.array as Float32Array, hit.instanceId * 16);
    const position = new THREE.Vector3();
    const scale = new THREE.Vector3();
    m.decompose(position, new THREE.Quaternion(), scale);
    picked = { model: group.model, position: [position.x, position.y, position.z], scale: scale.x };
  }

  /** Double click moves the focus point (and the area props load in) to the clicked spot. */
  function onDoubleClick(event: MouseEvent) {
    const hit = pick(event);
    if (!hit) return;
    const offset = camera.position.clone().sub(controls.target);
    controls.target.copy(hit.point);
    camera.position.copy(hit.point).add(offset);
    controls.update();
    scheduleUpdate();
  }

  onMount(() => {
    try {
      renderer = new THREE.WebGLRenderer({ antialias: true, logarithmicDepthBuffer: true });
    } catch (e) {
      error = `WebGL is not available: ${errorMessage(e)}`;
      loading = false;
      return;
    }
    renderer.setPixelRatio(window.devicePixelRatio);
    container!.appendChild(renderer.domElement);
    renderer.domElement.addEventListener("click", onClick);
    renderer.domElement.addEventListener("dblclick", onDoubleClick);

    scene = new THREE.Scene();
    scene.background = new THREE.Color(0x9fb4c8);
    camera = new THREE.PerspectiveCamera(50, 1, 1, 100000);
    scene.add(new THREE.HemisphereLight(0xe4ecff, 0x4a4032, 1.3));
    const sun = new THREE.DirectionalLight(0xfff4e0, 1.9);
    sun.position.set(0.6, 1, 0.35);
    scene.add(sun);
    scene.add(root);

    controls = new MapControls(camera, renderer.domElement);
    controls.enableDamping = false;
    controls.maxPolarAngle = Math.PI * 0.49;
    controls.zoomToCursor = true;
    controls.addEventListener("change", () => {
      requestRender();
      scheduleUpdate();
    });

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
      .loadMap(mft)
      .then((map) => {
        if (stale) return;
        const { heights: _h, propModels: _m, props: _p, ...rest } = map;
        header = rest;
        buildTerrain(map);
        buildWater(map);
        buildProps(map);
        root.updateMatrixWorld(true);
        frameMap(map);
      })
      .catch((e) => !stale && (error = errorMessage(e)))
      .finally(() => !stale && (loading = false));

    return () => {
      stale = true;
      disposed = true;
      clearTimeout(updateTimer);
      resize.disconnect();
      cancelAnimationFrame(frame);
      controls.dispose();
      root.traverse((o) => {
        if (o instanceof THREE.Mesh || o instanceof THREE.Points) o.geometry.dispose();
        if (o instanceof THREE.InstancedMesh) o.dispose();
      });
      for (const m of texturedReady.values()) {
        m.map?.dispose();
        m.dispose();
      }
      plainMaterial.dispose();
      terrainMaterial?.dispose();
      renderer?.dispose();
      renderer?.domElement.remove();
      renderer = null;
    };
  });

  $effect(() => {
    void [showProps, drawDistance, showAll];
    scheduleUpdate();
  });

  $effect(() => {
    void [useTextures, alphaCutout];
    applyMaterials();
  });

  $effect(() => {
    if (markers) markers.visible = showMarkers;
    if (water) water.visible = showWater;
    if (terrainMaterial) terrainMaterial.wireframe = wireframe;
    requestRender();
  });

  const size = $derived(header ? [header.rect[2] - header.rect[0], header.rect[3] - header.rect[1]] : [0, 0]);
  const formatSize = (n: number) => (n >= 1 << 20 ? `${(n / (1 << 20)).toFixed(1)} MiB` : `${(n / 1024).toFixed(1)} KiB`);
</script>

<div class="bar">
  <button class="btn small ghost" onclick={resetView} disabled={!header}>Reset view</button>
  <label><input type="checkbox" bind:checked={showProps} /> Props</label>
  <label title="Draw distance around the focus point; double-click to move it">
    <input type="range" min="2000" max="50000" step="1000" bind:value={drawDistance} disabled={!showProps || showAll} />
    <span class="mono distance">{showAll ? "all" : `${(drawDistance / 1000).toFixed(0)}k`}</span>
  </label>
  <label title="Load and draw every prop of the map regardless of distance. Large maps load thousands of models and draw over 100 million triangles.">
    <input type="checkbox" bind:checked={showAll} disabled={!showProps} /> All props
  </label>
  <label><input type="checkbox" bind:checked={useTextures} disabled={!showProps} /> Textures</label>
  <label title="Hide pixels with low texture alpha (foliage). Off by default: many textures use alpha for other purposes.">
    <input type="checkbox" bind:checked={alphaCutout} disabled={!showProps || !useTextures} /> Alpha cutout
  </label>
  <label><input type="checkbox" bind:checked={showMarkers} /> Markers</label>
  {#if header?.water}
    <label><input type="checkbox" bind:checked={showWater} /> Water</label>
  {/if}
  <label><input type="checkbox" bind:checked={wireframe} /> Wireframe</label>
  <button class="btn small ghost" onclick={() => (showInfo = !showInfo)} disabled={!header}>Info</button>
  {#if header}
    <span class="mono muted info">
      {drawn.props.toLocaleString()} / {header.propCount.toLocaleString()} props · {drawn.models.toLocaleString()} models ·
      {drawn.triangles.toLocaleString()} triangles{pending ? ` · loading ${pending}` : ""}
    </span>
  {/if}
</div>

<div class="stage" bind:this={container}>
  {#if loading}
    <div class="overlay muted">Loading map…</div>
  {:else if error}
    <div class="overlay error">{error}</div>
  {/if}
  {#if showInfo && header}
    <div class="panel">
      <dl>
        <dt>Size</dt>
        <dd>{size[0].toLocaleString()} × {size[1].toLocaleString()} units</dd>
        <dt>Terrain</dt>
        <dd>
          {#if header.columns}
            {header.columns} × {header.rows} samples{header.zRange
              ? `, height ${Math.round(-header.zRange[1]).toLocaleString()} to ${Math.round(-header.zRange[0]).toLocaleString()}`
              : ""}
          {:else}
            none
          {/if}
        </dd>
        <dt>Props</dt>
        <dd>
          {header.propCount.toLocaleString()} using {header.models.length.toLocaleString()} models
          <span class="muted">
            ({header.propKinds[0].toLocaleString()} static, {header.propKinds[1].toLocaleString()} animated,
            {header.propKinds[2].toLocaleString()} instanced, {header.propKinds[3].toLocaleString()} grouped{header.missingModels
              ? `; ${header.missingModels.toLocaleString()} without model`
              : ""})
          </span>
        </dd>
        {#if failed}
          <dt>Failed</dt>
          <dd>{failed} models could not be read</dd>
        {/if}
        <dt>Water</dt>
        <dd>
          {#if header.water}
            {header.water.plane ? "sea level plane" : "no plane"}{header.water.surfaces.length
              ? `, ${header.water.surfaces.length} surfaces`
              : ""}
          {:else}
            none
          {/if}
        </dd>
      </dl>
      <div class="chunks">
        {#each header.chunks as c (c.name)}
          <span class="mono">{c.name} <span class="muted">v{c.version} · {formatSize(c.size)}</span></span>
        {/each}
      </div>
    </div>
  {/if}
  {#if picked}
    <div class="picked">
      <span>Model <span class="mono">{picked.model}</span></span>
      <span class="muted mono">
        at {picked.position.map((v) => Math.round(v).toLocaleString()).join(", ")} · scale {picked.scale.toFixed(2)}
      </span>
      <button class="btn small ghost" onclick={() => (picked = null)} aria-label="Close">✕</button>
    </div>
  {/if}
</div>
<div class="hint muted">
  Drag to pan · right-drag to rotate · scroll to zoom · double-click to move the focus point (props load around it) · click a
  prop to identify it
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

  input[type="range"] {
    accent-color: var(--accent);
    width: 110px;
  }

  .distance {
    min-width: 28px;
  }

  .info {
    margin-left: auto;
  }

  .stage {
    flex: 1;
    position: relative;
    min-height: 0;
    overflow: hidden;
    background: #9fb4c8;
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
    background: var(--bg);
  }

  .error {
    color: var(--accent);
  }

  .panel,
  .picked {
    position: absolute;
    left: 12px;
    padding: 10px 12px;
    border-radius: 6px;
    background: rgba(18, 20, 24, 0.88);
    color: #e6e8ec;
    font-size: 12px;
  }

  .panel {
    top: 12px;
    max-width: min(460px, calc(100% - 24px));
  }

  dl {
    display: grid;
    grid-template-columns: auto 1fr;
    gap: 4px 12px;
    margin: 0 0 8px;
  }

  dt {
    color: #9aa1ad;
  }

  dd {
    margin: 0;
  }

  .chunks {
    display: flex;
    flex-wrap: wrap;
    gap: 4px 12px;
    font-size: 11px;
  }

  .picked {
    bottom: 12px;
    display: flex;
    align-items: center;
    gap: 10px;
  }

  .hint {
    padding: 4px 12px;
    font-size: 11px;
    background: var(--panel);
    border-top: 1px solid var(--border);
  }
</style>
