import {
  AppWindow,
  AudioLines,
  Binary,
  Bone,
  Box,
  CaseSensitive,
  Clapperboard,
  Cpu,
  Database,
  DoorOpen,
  Earth,
  File,
  FileMusic,
  FileQuestionMark,
  FileStack,
  FileText,
  Film,
  Folder,
  Hand,
  Image,
  ImageOff,
  Languages,
  Library,
  ListMusic,
  ListVideo,
  Map,
  MapPin,
  MoonStar,
  Mic,
  Mountain,
  Network,
  Package,
  Palette,
  PersonStanding,
  Puzzle,
  Rotate3d,
  Scale,
  ScrollText,
  Settings,
  ShieldCheck,
  SlidersHorizontal,
  SquareDashed,
  SunDim,
  Type,
} from "@lucide/svelte";

/** Colour group of an icon; see the `--tone-*` variables in app.css. */
export type Tone = "image" | "sound" | "model" | "animation" | "text" | "map" | "font" | "video" | "code" | "data" | "unknown";

export interface IconSpec {
  icon: typeof File;
  tone: Tone;
  label: string;
}

const spec = (icon: typeof File, tone: Tone, label: string): IconSpec => ({ icon, tone, label });

/** Icon per file type name (as sent by the backend). */
const byType: Record<string, IconSpec> = {
  ATEX: spec(Image, "image", "Texture"),
  ATTX: spec(Image, "image", "Terrain texture"),
  ATEC: spec(Image, "image", "Texture"),
  ATEP: spec(Image, "image", "Map texture"),
  ATEU: spec(Image, "image", "UI texture"),
  ATET: spec(Image, "image", "Texture"),
  CTEX: spec(ImageOff, "image", "Texture (unsupported format)"),
  DDS: spec(Image, "image", "DDS image"),
  JPEG: spec(Image, "image", "JPEG image"),
  WebP: spec(Image, "image", "WebP image"),
  PNG: spec(Image, "image", "PNG image"),

  sound: spec(FileMusic, "sound", "Sound (unknown codec)"),
  asndMP3: spec(AudioLines, "sound", "MP3 sound"),
  asndOgg: spec(AudioLines, "sound", "Ogg sound"),
  PackedMP3: spec(AudioLines, "sound", "MP3 sound"),
  PackedOgg: spec(AudioLines, "sound", "Ogg sound"),
  Ogg: spec(AudioLines, "sound", "Ogg sound"),
  MP3: spec(AudioLines, "sound", "MP3 sound"),
  ABNK: spec(Library, "sound", "Sound bank"),
  ABIX: spec(ListMusic, "sound", "Sound bank index"),
  AMSP: spec(ScrollText, "sound", "Audio script"),
  RIFF: spec(File, "data", "RIFF container"),

  MODL: spec(Box, "model", "3D model"),
  "MODL animated": spec(Rotate3d, "model", "Model with animations"),
  "MODL static": spec(Box, "model", "Model without animations"),
  "MODL skeleton": spec(Bone, "animation", "Skeleton (no meshes)"),
  "MODL animset": spec(ListVideo, "animation", "Animation set for a skeleton (no meshes)"),
  "MODL materials": spec(Palette, "model", "Materials only (no meshes)"),
  "MODL empty": spec(SquareDashed, "unknown", "Empty model file"),
  bone: spec(SlidersHorizontal, "animation", "Bone scales (character creation)"),
  "MODL anim": spec(PersonStanding, "animation", "Animation clip"),
  anic: spec(ListVideo, "animation", "Animation sequences"),
  emoc: spec(Hand, "animation", "Emote animations"),
  cmaC: spec(ShieldCheck, "model", "Model collision manifest"),

  mapc: spec(Map, "map", "Map"),
  hvkC: spec(Mountain, "map", "Map collision"),
  mpsd: spec(SunDim, "map", "Map shadows"),
  mMet: spec(MapPin, "map", "Map metadata"),
  PIMG: spec(Earth, "map", "World map tiles"),
  prlt: spec(DoorOpen, "map", "Portal manifest"),
  STAR: spec(MoonStar, "map", "Star field (night sky)"),

  strs: spec(Languages, "text", "Strings"),
  eula: spec(Scale, "text", "License agreement"),
  text: spec(FileText, "text", "Text"),
  "UTF-8": spec(FileText, "text", "Text (UTF-8)"),
  txtm: spec(FileStack, "text", "Text pack manifest"),
  txtV: spec(FileStack, "text", "Text pack variant"),
  txtv: spec(Mic, "text", "Text pack voices"),
  cntc: spec(Database, "data", "Game content"),
  ARMF: spec(FileStack, "data", "Manifest"),
  DEPS: spec(Network, "data", "Dependency table"),
  locl: spec(Settings, "data", "Configuration"),

  AFNT: spec(Type, "font", "Bitmap font"),
  "AFNT glyphs": spec(CaseSensitive, "font", "Bitmap font glyphs"),
  font: spec(Type, "font", "TrueType font"),

  CINP: spec(Clapperboard, "video", "Cinematic"),
  Bink2: spec(Film, "video", "Bink 2 video"),

  AMAT: spec(Palette, "code", "Material"),
  CDHS: spec(Cpu, "code", "Shader cache"),
  cmpc: spec(PersonStanding, "model", "Character composition"),
  EXE: spec(AppWindow, "code", "Executable"),
  DLL: spec(Puzzle, "code", "Library (DLL)"),
  binary: spec(Binary, "code", "Binary"),

  PF: spec(Package, "data", "PF container"),
  ARAP: spec(Package, "data", "ARAP data"),
  unknown: spec(FileQuestionMark, "unknown", "Unknown"),
};

const fallback = spec(File, "unknown", "File");
const folder = spec(Folder, "unknown", "Folder");

/** Generic icon per kind, for categories that mix several file types. */
const byKind: Record<string, IconSpec> = {
  image: spec(Image, "image", "Images"),
  sound: spec(AudioLines, "sound", "Sounds"),
  soundbank: spec(Library, "sound", "Sound banks"),
  soundbankindex: spec(ListMusic, "sound", "Sound bank indexes"),
  audioscript: spec(ScrollText, "sound", "Audio scripts"),
  pagedimage: spec(Earth, "map", "World map tiles"),
  emotes: spec(Hand, "animation", "Emote animations"),
  collisionmanifest: spec(ShieldCheck, "model", "Model collision manifest"),
  portalmanifest: spec(DoorOpen, "map", "Portal manifests"),
  collision: spec(Mountain, "map", "Collision"),
  mapshadow: spec(SunDim, "map", "Map shadows"),
  mapmetadata: spec(MapPin, "map", "Map metadata"),
  composite: spec(PersonStanding, "model", "Character composition"),
  model: spec(Box, "model", "Models"),
  animation: spec(PersonStanding, "animation", "Animations"),
  strings: spec(Languages, "text", "Strings"),
  text: spec(FileText, "text", "Text"),
  eula: spec(Scale, "text", "License agreements"),
  content: spec(Database, "data", "Game content"),
  bitmapfont: spec(Type, "font", "Fonts"),
  fontglyphs: spec(CaseSensitive, "font", "Font glyphs"),
  font: spec(Type, "font", "Fonts"),
  starfield: spec(MoonStar, "map", "Star fields"),
  sequences: spec(ListVideo, "animation", "Animation sequences"),
  textpack: spec(Languages, "text", "Text packs"),
  map: spec(Map, "map", "Maps"),
  material: spec(Palette, "code", "Materials"),
  binary: spec(Binary, "code", "Binary data"),
};

/** Icon of a category: its file type, or its kind when it mixes types. */
export function iconForCategory(category: { content: string | null; contentKind: string | null; mixed: boolean }): IconSpec {
  if (category.mixed && category.contentKind) return byKind[category.contentKind] ?? iconFor(category.content);
  return iconFor(category.content);
}

export function iconFor(type: string | null | undefined): IconSpec {
  if (!type) return folder;
  return byType[type] ?? fallback;
}
