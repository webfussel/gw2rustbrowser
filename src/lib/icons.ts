import {
  AppWindow,
  AudioLines,
  Binary,
  Box,
  CaseSensitive,
  Clapperboard,
  Cpu,
  Database,
  DoorOpen,
  File,
  FileMusic,
  FileQuestionMark,
  FileStack,
  FileText,
  Film,
  Folder,
  Grid3x3,
  Hand,
  Image,
  ImageOff,
  Languages,
  Layers,
  Library,
  ListMusic,
  ListVideo,
  Map,
  MapPin,
  Mic,
  Mountain,
  Network,
  Package,
  Palette,
  PersonStanding,
  Puzzle,
  Scale,
  ScrollText,
  Settings,
  ShieldCheck,
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
  "MODL anim": spec(PersonStanding, "animation", "Animation clip"),
  anic: spec(ListVideo, "animation", "Animation sequences"),
  emoc: spec(Hand, "animation", "Emote animations"),
  cmaC: spec(ShieldCheck, "model", "Model collision manifest"),

  mapc: spec(Map, "map", "Map"),
  hvkC: spec(Mountain, "map", "Map collision"),
  mpsd: spec(SunDim, "map", "Map shadows"),
  mMet: spec(MapPin, "map", "Map metadata"),
  PIMG: spec(Grid3x3, "map", "Paged image table"),
  prlt: spec(DoorOpen, "map", "Portal manifest"),

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
  font: spec(Type, "font", "Font"),

  CINP: spec(Clapperboard, "video", "Cinematic"),
  Bink2: spec(Film, "video", "Bink 2 video"),

  AMAT: spec(Palette, "code", "Material"),
  CDHS: spec(Cpu, "code", "Shader cache"),
  cmpc: spec(Layers, "code", "Composite data"),
  EXE: spec(AppWindow, "code", "Executable"),
  DLL: spec(Puzzle, "code", "Library (DLL)"),
  binary: spec(Binary, "code", "Binary"),

  PF: spec(Package, "data", "PF container"),
  ARAP: spec(Package, "data", "ARAP data"),
  unknown: spec(FileQuestionMark, "unknown", "Unknown"),
};

const fallback = spec(File, "unknown", "File");
const folder = spec(Folder, "unknown", "Folder");

export function iconFor(type: string | null | undefined): IconSpec {
  if (!type) return folder;
  return byType[type] ?? fallback;
}
