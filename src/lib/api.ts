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
  | "model"
  | "animation"
  | "strings"
  | "text"
  | "eula"
  | "content"
  | "bitmapfont"
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
  positions: Float32Array<ArrayBuffer>;
  normals: Float32Array<ArrayBuffer> | null;
  uvs: Float32Array<ArrayBuffer> | null;
  indices: Uint32Array<ArrayBuffer>;
  materialIndex: number;
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

export interface ModelSummary {
  bones: number;
  meshes: number;
  animations: number;
  materials: number;
  textured: boolean;
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
    const indices = new Uint32Array(buffer, offset, indexCount);
    offset += indexCount * 4;
    meshes.push({ positions, normals, uvs, indices, materialIndex });
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
  modelSummary: (mft: number) => invoke<ModelSummary>("model_summary", { mft }),
  loadFonts: (mft: number) => invoke<FontInfo[]>("load_fonts", { mft }),
  fontAtlas: async (mft: number, font: number, range: number, page: number) => {
    const image = parseImage(await invoke<ArrayBuffer>("font_atlas", { mft, font, range, page }));
    const pages = Number(image.format.split("|")[1] ?? 1);
    return { image: { ...image, format: "AFNT glyph atlas" }, pages };
  },
  modelMaterials: (mft: number) => invoke<ModelMaterial[]>("model_materials", { mft }),
  loadSound: async (mft: number) => parseSound(await invoke<ArrayBuffer>("load_sound", { mft })),
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
