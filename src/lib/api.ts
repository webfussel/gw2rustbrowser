import { invoke } from "@tauri-apps/api/core";

export interface DatSummary {
  path: string;
  files: number;
  categories: number;
  rescanned: number;
  fromCache: boolean;
  millis: number;
}

export interface Category {
  id: number;
  name: string;
  parent: number | null;
  total: number;
  files: number;
  /** Representative file type below this category. */
  content: string | null;
  contentKind: Kind | null;
  /** Whether the category contains several file types. */
  mixed: boolean;
}

export interface FileInfo {
  mft: number;
  baseId: number;
  fileId: number;
  type: string;
  size: number;
  knownName?: string;
  /** Original asset path from development (models and their collision files). */
  assetPath?: string;
}

export interface EntryDetail extends FileInfo {
  kind: Kind;
  category: number;
  path: string[];
  storedSize: number;
  offset: number;
  compressed: boolean;
  convertedExtension: string;
  rawExtension: string;
}

export type Kind =
  | "image"
  | "sound"
  | "soundbank"
  | "soundbankindex"
  | "audioscript"
  | "pagedimage"
  | "emotes"
  | "collisionmanifest"
  | "portalmanifest"
  | "model"
  | "animation"
  | "strings"
  | "text"
  | "eula"
  | "content"
  | "bitmapfont"
  | "fontglyphs"
  | "font"
  | "starfield"
  | "sequences"
  | "textpack"
  | "map"
  | "material"
  | "binary";

export interface FindResult extends FileInfo {
  category: number;
  path: string[];
}

export interface DecodedImage {
  width: number;
  height: number;
  format: string;
  pixels: Uint8ClampedArray<ArrayBuffer>;
}

export interface SoundData {
  mime: string;
  data: Uint8Array<ArrayBuffer>;
}

export interface BankEntry {
  index: number;
  voiceId: number;
  codec: string | null;
  size: number;
  length: number | null;
  error: string | null;
}

export interface StringsData {
  language: string;
  entries: [number, string | null][];
}

export interface EulaText {
  language: string;
  text: string;
}

export interface ModelMesh {
  /** In the file's coordinate space (see ModelViewer). */
  positions: Float32Array<ArrayBuffer>;
  normals: Float32Array<ArrayBuffer> | null;
  uvs: Float32Array<ArrayBuffer> | null;
  /** Four skeleton bone indices per vertex. */
  skinIndices: Uint16Array<ArrayBuffer> | null;
  /** Four weights per vertex, 0–255. */
  skinWeights: Uint8Array<ArrayBuffer> | null;
  indices: Uint32Array<ArrayBuffer>;
  materialIndex: number;
}

export interface RigBone {
  name: string;
  parent: number | null;
  translation: [number, number, number];
  rotation: [number, number, number, number];
  scale: [number, number, number];
}

export interface AnimationRef {
  source: number;
  index: number;
  name: string;
  group: string;
}

export interface Rig {
  bones: RigBone[];
  skeletonFile: number | null;
  animations: AnimationRef[];
}

export interface ClipTrack {
  bone: string;
  rotationTimes: number[];
  rotations: number[];
  translationTimes: number[];
  translations: number[];
  scaleTimes: number[];
  scales: number[];
}

export interface Clip {
  name: string;
  duration: number;
  tracks: ClipTrack[];
}

export interface FontRange {
  range: number;
  name: string;
  fileId: number;
}

export interface FontInfo {
  index: number;
  extentX: number;
  extentY: number;
  ranges: FontRange[];
}

export interface AnimationInfo {
  name: string;
  duration: number;
  timeStep: number;
  frames: number;
  oversampling: number;
  trackGroups: number;
  pointerBits: number;
  chunkVersion: number;
  dataSize: number;
  relocations: number;
}

export interface TextRow {
  id: number;
  text: string | null;
  state: "plain" | "encrypted" | "missing";
  voice: number | null;
  /** null while the voice index is not built. */
  playable: boolean | null;
}

export interface VariantRow extends TextRow {
  variants: TextRow[];
}

export interface TextPackInfo {
  languages: { index: number; name: string }[];
  textCount: number;
  stringsPerFile: number;
  variantCount: number;
  voiceCount: number;
  voiceIndexReady: boolean;
}

export interface SequenceStep {
  /** 0 = action, 1 = movement. */
  kind: number;
  durationMs: number;
  animationSpeed: number;
  flags: number;
  moveRotation?: number[];
  facingRotation?: number[];
  velocity?: number[];
}

export interface SequenceTrigger {
  kind: number;
  timeMs: number;
  flags: number;
}

export interface SequenceVariant {
  name: string;
  flags: number;
  blendIn: number;
  blendOut: number;
  steps: SequenceStep[];
  triggers: SequenceTrigger[];
  chargeStages: { duration: number; level: number }[];
}

export interface AnimSequence {
  name: string;
  variants: SequenceVariant[];
}

export interface StarFieldData {
  header: number;
  /** Six values per star: azimuth, elevation, value 2, brightness, value 4, value 5. */
  stars: number[];
}

export interface TrueTypeInfo {
  family: string;
  subfamily: string;
  fullName: string;
  version: string;
  glyphs: number;
  characters: number[];
}

export interface GlyphFileInfo {
  range: number;
  rangeName: string;
  first: number;
  last: number;
  fonts: number[];
}

export interface ModelSummary {
  bones: number;
  meshes: number;
  animations: number;
  materials: number;
  skeleton: number | null;
  users: number;
}

export interface ModelMaterial {
  index: number;
  diffuse: number | null;
  normal: number | null;
  materialFile: number | null;
}

function parseImage(buffer: ArrayBuffer): DecodedImage {
  const view = new DataView(buffer);
  const width = view.getUint32(0, true);
  const height = view.getUint32(4, true);
  const formatLength = view.getUint16(8, true);
  const format = new TextDecoder().decode(new Uint8Array(buffer, 10, formatLength));
  const pixels = new Uint8ClampedArray(buffer, 10 + formatLength, width * height * 4);
  return { width, height, format, pixels };
}

function parseModel(buffer: ArrayBuffer): ModelMesh[] {
  const view = new DataView(buffer);
  let offset = 0;
  const u32 = () => {
    const v = view.getUint32(offset, true);
    offset += 4;
    return v;
  };
  const floats = (count: number) => {
    const array = new Float32Array(buffer, offset, count);
    offset += count * 4;
    return array;
  };
  const meshes: ModelMesh[] = [];
  const count = u32();
  for (let i = 0; i < count; i++) {
    const vertices = u32();
    const indexCount = u32();
    const flags = u32();
    const materialIndex = u32();
    const positions = floats(vertices * 3);
    const normals = flags & 1 ? floats(vertices * 3) : null;
    const uvs = flags & 2 ? floats(vertices * 2) : null;
    let skinIndices: Uint16Array<ArrayBuffer> | null = null;
    let skinWeights: Uint8Array<ArrayBuffer> | null = null;
    if (flags & 4) {
      // Interleaved per vertex: four u16 bone indices, then four u8 weights.
      skinIndices = new Uint16Array(vertices * 4);
      skinWeights = new Uint8Array(vertices * 4);
      const bytes = new Uint8Array(buffer);
      for (let v = 0; v < vertices; v++) {
        const at = offset + v * 12;
        for (let j = 0; j < 4; j++) {
          skinIndices[v * 4 + j] = view.getUint16(at + j * 2, true);
          skinWeights[v * 4 + j] = bytes[at + 8 + j];
        }
      }
      offset += vertices * 12;
    }
    const indices = new Uint32Array(buffer, offset, indexCount);
    offset += indexCount * 4;
    meshes.push({ positions, normals, uvs, skinIndices, skinWeights, indices, materialIndex });
  }
  return meshes;
}

export interface MapWaterSurface {
  z: number;
  outline: [number, number][];
}

export interface MapHeader {
  /** Min x, min y, max x, max y in world units. */
  rect: [number, number, number, number];
  columns: number;
  rows: number;
  zRange: [number, number] | null;
  /** Model file id per prop model index. */
  models: number[];
  propCount: number;
  /** Static, animated, instance copies, group members. */
  propKinds: [number, number, number, number];
  missingModels: number;
  water: { plane: boolean; planeZ: number; surfaces: MapWaterSurface[] } | null;
  chunks: { name: string; version: number; size: number }[];
}

export interface MapData extends MapHeader {
  /** World z per terrain sample, row-major, row 0 at the minimum y. */
  heights: Float32Array<ArrayBuffer>;
  /** Per prop: model index, position xyz, rotation xyz, scale. */
  propModels: Uint32Array<ArrayBuffer>;
  props: Float32Array<ArrayBuffer>;
}

function parseMap(buffer: ArrayBuffer): MapData {
  const headerLength = new DataView(buffer).getUint32(0, true);
  const header: MapHeader = JSON.parse(new TextDecoder().decode(new Uint8Array(buffer, 4, headerLength)));
  let offset = Math.ceil((4 + headerLength) / 4) * 4;
  const heights = new Float32Array(buffer, offset, header.columns * header.rows);
  offset += heights.byteLength;
  return {
    ...header,
    heights,
    propModels: new Uint32Array(buffer, offset, header.propCount * 8),
    props: new Float32Array(buffer, offset, header.propCount * 8),
  };
}

export interface FileRef {
  id: number;
  /** MFT index, -1 if the file is not in the archive. */
  mft: number;
  kind: string | null;
}

export interface CollisionEntry {
  name: string;
  model: FileRef | null;
  collision: FileRef | null;
  scales: number[];
}

export interface PortalStart {
  token: number;
  position: [number, number, number];
  radius: number;
  models: number;
  /** Models per file type name. */
  kinds: Record<string, number>;
}

export interface PortalMap {
  guid: string;
  redirectors: { guid: string; token: number; position: [number, number, number]; radius: number }[];
  starts: PortalStart[];
}

export interface PortalModel {
  file: FileRef | null;
  flags: number;
  kind: number;
  permutation: string;
}

export interface EmoteTiming {
  fileId: number | null;
  /** MFT index of the model file, -1 if missing. */
  mft: number;
  fileType: string | null;
  /** Seconds. */
  blendIn: number;
  blendOut: number;
  /** Milliseconds. */
  intro: number;
  loopLength: number;
  outro: number;
  startOffset: number;
}

export interface Emote {
  name: string;
  timings: EmoteTiming[];
}

export interface PagedImageLayer {
  index: number;
  pageWidth: number;
  pageHeight: number;
  format: string;
  /** [min x, min y, max x, max y] of the pages. */
  bounds: [number, number, number, number] | null;
  xs: number[];
  ys: number[];
  /** Texture file id per page, 0 for a solid page. */
  files: number[];
  /** Solid page colour, 0xRRGGBBAA. */
  colors: number[];
}

export interface ScriptFile {
  id: number;
  /** MFT index, -1 if the file is not in the archive. */
  mft: number;
  kind: string | null;
}

export interface ScriptSoundFile {
  file: ScriptFile | null;
  volume: number;
  weight: number;
  condition: string | null;
  language: string | null;
}

export interface ScriptSound {
  name: string;
  category: string | null;
  files: ScriptSoundFile[];
  volume: string;
  pitch: string;
  pan: string;
  fadeIn: number;
  fadeOut: number;
  loopCount: number;
  flags: number;
  channelMax: number;
  priority: number;
  playbackMode: number;
  positionMode: number;
  fileIterateMode: number;
  loopMode: number;
  dspEffects: number;
  attenuation: boolean;
  initialDelay: string | null;
  repeatCount: string | null;
  repeatTime: string | null;
  playLength: string | null;
  endCue: string | null;
  offsetBone: string | null;
  playlist: string | null;
}

export interface ScriptPlaylist {
  name: string;
  category: string | null;
  primary: string | null;
  secondary: string | null;
  files: ScriptSoundFile[];
  volume: string;
  fadeIn: number;
  fadeOut: number;
  flags: number;
  fileIterateMode: number;
  initialSilence: string | null;
  intervalSilence: string | null;
}

export interface AudioScriptInfo {
  version: number;
  volume: number;
  fadeIn: number;
  flags: number;
  soundPoolCount: number;
  soundPoolDelay: number;
  musicCue: string | null;
  reverbOverride: string | null;
  snapshot: string | null;
  sounds: ScriptSound[];
  triggers: { name: string; markers: { time: number; cue: string | null; end: string | null; kind: number }[] }[];
  handlers: { name: string; flags: number; size: number; code: string }[];
  scriptRefs: { name: string; file: ScriptFile | null }[];
  properties: { name: string; token: string | null; value: number }[];
  settings: {
    defaultSnapshot: string | null;
    distanceScale: number;
    dopplerScale: number;
    busses: number;
    categories: number;
    materials: number;
    reverbs: number;
    snapshots: number;
    playlists: ScriptPlaylist[];
    musicExternal: [string, string][];
    bankIndex: ScriptFile | null;
    bankScript: ScriptFile | null;
    musicScript: ScriptFile | null;
  } | null;
}

export interface BankIndexLanguage {
  code: number;
  name: string;
  /** Bank file id per bank number, 0 for none. */
  files: number[];
  /** MFT index of each bank file, -1 if missing. */
  mfts: number[];
}

export interface MaterialEffect {
  name: string;
  renderState: string;
  /** Readable render state parts (blend, depth, culling…). */
  state: string[];
  passFlags: number;
  pixelShader: number;
  /** [vertex variant, shader index]. */
  vertexShaders: [number, number][];
}

export interface MaterialInfo {
  version: number;
  settings: {
    textures: string[];
    textureCount: number;
    textureArrayRange: number;
    sortOrder: number;
    sortTriangles: number;
    proceduralAnimation: number;
    flags: number;
    debugFlags: number;
  } | null;
  techniques: { id: number; passes: MaterialEffect[][] }[];
  /** Readable sampler state per sampler slot. */
  samplers: string[];
  shaders: { stage: string; size: number; profile: string | null; instructions: number; uniforms: number }[];
}

export interface ShaderSignature {
  semantic: string;
  register: number;
  mask: string;
  componentType: string;
  systemValue: number;
}

export interface MaterialShader {
  stage: string;
  bgfxVersion: number;
  hashIn: number;
  hashOut: number;
  uniforms: { name: string; kind: string; count: number; register: number; registerCount: number }[];
  attributes: string[];
  constantBufferSize: number;
  inputs: ShaderSignature[];
  outputs: ShaderSignature[];
  profile: string | null;
  lines: { text: string; depth: number; note: string | null }[];
  error: string | null;
  samplers: string[];
}

export interface PropMesh {
  positions: Float32Array<ArrayBuffer>;
  normals: Float32Array<ArrayBuffer> | null;
  uvs: Float32Array<ArrayBuffer> | null;
  indices: Uint32Array<ArrayBuffer>;
  /** Diffuse texture file id. */
  diffuse: number | null;
}

function parsePropModel(buffer: ArrayBuffer): PropMesh[] {
  const view = new DataView(buffer);
  let offset = 0;
  const u32 = () => {
    const v = view.getUint32(offset, true);
    offset += 4;
    return v;
  };
  const floats = (count: number) => {
    const array = new Float32Array(buffer, offset, count);
    offset += count * 4;
    return array;
  };
  const meshes: PropMesh[] = [];
  const count = u32();
  for (let i = 0; i < count; i++) {
    const vertices = u32();
    const indexCount = u32();
    const flags = u32();
    const diffuse = u32();
    const positions = floats(vertices * 3);
    const normals = flags & 1 ? floats(vertices * 3) : null;
    const uvs = flags & 2 ? floats(vertices * 2) : null;
    const indices = new Uint32Array(buffer, offset, indexCount);
    offset += indexCount * 4;
    meshes.push({ positions, normals, uvs, indices, diffuse: diffuse || null });
  }
  return meshes;
}

function parseSound(buffer: ArrayBuffer): SoundData {
  const bytes = new Uint8Array(buffer);
  const mime = new TextDecoder().decode(bytes.subarray(1, 1 + bytes[0]));
  return { mime, data: bytes.subarray(1 + bytes[0]) };
}

export interface ScanProgress {
  done: number;
  total: number;
}

export interface ExportProgress {
  done: number;
  total: number;
  failed: number;
}

export interface ExportSummary {
  written: number;
  failed: number;
  cancelled: boolean;
  errors: string[];
  directory: string;
}

export const api = {
  startupPath: () => invoke<string | null>("startup_path"),
  openDat: (path: string) => invoke<DatSummary>("open_dat", { path }),
  categories: () => invoke<Category[]>("categories"),
  categoryFiles: (category: number) => invoke<FileInfo[]>("category_files", { category }),
  entryInfo: (mft: number) => invoke<EntryDetail>("entry_info", { mft }),
  find: (query: string) => invoke<FindResult[]>("find", { query }),
  readFile: async (mft: number) => new Uint8Array(await invoke<ArrayBuffer>("read_file", { mft })),
  loadImage: async (mft: number) => parseImage(await invoke<ArrayBuffer>("load_image", { mft })),
  loadTexture: async (fileId: number) => parseImage(await invoke<ArrayBuffer>("load_texture", { fileId })),
  loadText: (mft: number) => invoke<string>("load_text", { mft }),
  loadAnimation: (mft: number) => invoke<AnimationInfo>("load_animation", { mft }),
  textpackInfo: () => invoke<TextPackInfo>("textpack_info"),
  textRows: (language: number, start: number, count: number) => invoke<TextRow[]>("text_rows", { language, start, count }),
  variantRows: (language: number, start: number, count: number) =>
    invoke<VariantRow[]>("variant_rows", { language, start, count }),
  voiceRows: (language: number, start: number, count: number, playableOnly: boolean) =>
    invoke<{ total: number; rows: TextRow[] }>("voice_rows", { language, start, count, playableOnly }),
  searchTexts: (language: number, query: string) => invoke<TextRow[]>("search_texts", { language, query }),
  buildVoiceIndex: () => invoke<{ entries: number; playable: number }>("build_voice_index"),
  playVoice: async (voice: number) => parseSound(await invoke<ArrayBuffer>("play_voice", { voice })),
  loadSequences: async (mft: number): Promise<AnimSequence[]> =>
    JSON.parse(new TextDecoder().decode(await invoke<ArrayBuffer>("load_sequences", { mft }))),
  loadStars: (mft: number) => invoke<StarFieldData>("load_stars", { mft }),
  materialInfo: (mft: number) => invoke<MaterialInfo>("material_info", { mft }),
  materialShader: (mft: number, index: number) => invoke<MaterialShader>("material_shader", { mft, index }),
  loadMap: async (mft: number) => parseMap(await invoke<ArrayBuffer>("load_map", { mft })),
  loadPropModel: async (fileId: number) => parsePropModel(await invoke<ArrayBuffer>("load_prop_model", { fileId })),
  truetypeInfo: (mft: number) => invoke<TrueTypeInfo>("truetype_info", { mft }),
  glyphFileInfo: (mft: number) => invoke<GlyphFileInfo>("glyph_file_info", { mft }),
  glyphFileAtlas: async (mft: number, page: number) => {
    const image = parseImage(await invoke<ArrayBuffer>("glyph_file_atlas", { mft, page }));
    const [, pages, glyphs] = image.format.split("|").map(Number);
    return { image: { ...image, format: `${glyphs} glyphs` }, pages };
  },
  modelRig: (mft: number) => invoke<Rig>("model_rig", { mft }),
  modelClip: (source: number, index: number) => invoke<Clip>("model_clip", { source, index }),
  modelSummary: (mft: number) => invoke<ModelSummary>("model_summary", { mft }),
  loadFonts: (mft: number) => invoke<FontInfo[]>("load_fonts", { mft }),
  fontAtlas: async (mft: number, font: number, range: number, page: number) => {
    const image = parseImage(await invoke<ArrayBuffer>("font_atlas", { mft, font, range, page }));
    const pages = Number(image.format.split("|")[1] ?? 1);
    return { image: { ...image, format: "AFNT glyph atlas" }, pages };
  },
  modelMaterials: (mft: number) => invoke<ModelMaterial[]>("model_materials", { mft }),
  loadSound: async (mft: number) => parseSound(await invoke<ArrayBuffer>("load_sound", { mft })),
  collisionManifest: (mft: number) => invoke<CollisionEntry[]>("collision_manifest_info", { mft }),
  portalManifest: (mft: number) => invoke<PortalMap[]>("portal_manifest_info", { mft }),
  portalStartModels: (mft: number, map: number, start: number) =>
    invoke<PortalModel[]>("portal_start_models", { mft, map, start }),
  emoteInfo: (mft: number) => invoke<Emote[]>("emote_info", { mft }),
  pagedImageInfo: (mft: number) => invoke<PagedImageLayer[]>("paged_image_info", { mft }),
  pagedImageOverview: async (mft: number, layer: number, maxSize: number) =>
    parseImage(await invoke<ArrayBuffer>("paged_image_overview", { mft, layer, maxSize })),
  pagedImageTile: async (fileId: number, size: number) =>
    parseImage(await invoke<ArrayBuffer>("paged_image_tile", { fileId, size })),
  exportPagedImage: (mft: number, layer: number, pagePixels: number, path: string) =>
    invoke<void>("export_paged_image", { mft, layer, pagePixels, path }),
  audioScriptInfo: (mft: number) => invoke<AudioScriptInfo>("audio_script_info", { mft }),
  loadBankIndex: (mft: number) => invoke<BankIndexLanguage[]>("load_bank_index", { mft }),
  loadBank: (mft: number) => invoke<BankEntry[]>("load_bank", { mft }),
  loadBankSound: async (mft: number, index: number) => parseSound(await invoke<ArrayBuffer>("load_bank_sound", { mft, index })),
  loadStrings: (mft: number) => invoke<StringsData>("load_strings", { mft }),
  loadEula: (mft: number) => invoke<EulaText[]>("load_eula", { mft }),
  loadModel: async (mft: number) => parseModel(await invoke<ArrayBuffer>("load_model", { mft })),
  exportBankSound: (mft: number, index: number, path: string) => invoke<void>("export_bank_sound", { mft, index, path }),
  suggestFileName: (mft: number, converted: boolean) => invoke<string>("suggest_file_name", { mft, converted }),
  exportFile: (mft: number, path: string, converted: boolean) => invoke<void>("export_file", { mft, path, converted }),
  exportFiles: (args: { category?: number; files?: number[]; directory: string; converted: boolean }) =>
    invoke<ExportSummary>("export_files", {
      category: args.category ?? null,
      files: args.files ?? null,
      directory: args.directory,
      converted: args.converted,
    }),
  cancelExport: () => invoke<void>("cancel_export"),
};
