# GW2 Browser (Rust)

## Prologue

Everything this project can do, it can do because of the original projects.
**Absolutely nothing here, 0%, would have been possible without the incredible
work of the original creators.** Their authors reverse-engineered the `Gw2.dat` archive, its compression and
its file formats by hand, at a time when none of it was documented anywhere.
That work is the hard part, it is brilliant, and it was shared with everyone
for free. This repository only stands on their shoulders.

**The original projects:**

| Project | What it is | Repository |
|---|---|---|
| **Gw2Browser** | Browser for the contents of `Gw2.dat`, the application this port is based on (Rhoot, Khralkatorrix and others; maintained fork by kytulendu) | [rhoot/Gw2Browser](https://github.com/rhoot/Gw2Browser), fork: [kytulendu/gw2browser](https://github.com/kytulendu/gw2browser) |
| **gw2DatTools** | Reading and decompressing the `.dat` archive and its textures (Antoine Hom) | [ahom/gw2_reverse](https://github.com/ahom/gw2_reverse), fork: [kytulendu/gw2dattools](https://github.com/kytulendu/gw2dattools) |
| **gw2formats** | Type-safe descriptions of the game's file formats (Rhoot, Khralkatorrix) | [rhoot/gw2formats](https://github.com/rhoot/gw2formats), fork: [kytulendu/gw2formats](https://github.com/kytulendu/gw2formats) |

And yes: checking out somebody else's carefully built C++ source and going
"let's rewrite it in Rust" felt incredibly dirty. It is about the most
stereotypical thing one can do with someone else's project, and I am fully
aware of it. And yes, that includes the text you are reading right now: **even this README was
generated - except for this prologue which was written by my own hands.**

**If you want to support anything here, support the original projects** —
star them, contribute to them, credit them.

Please do not consider opening PRs for this. I will **maybe** continue work on it, but I will decline any PR done.
My only hope is it can serve as a reference to improve the original tool which, like I already said, **deserve every attention.**

All of it started as an experiment, that evolved out of boredom.
My goal also was not "Learn Rust" or "Learn Tauri" or anything, because I already know those.
In the end, I just wanted to provide a fast and stable tool to the community,
so that maybe more people can do datamining, and it becomes more popular for this game.

# Readme
Rust port of [Gw2Browser](https://github.com/kytulendu/gw2browser): opens the
`Gw2.dat` of Guild Wars 2, shows its contents and exports them.

- **Core library** `crates/gw2dat`: pure Rust, no C dependencies
- **App** `src-tauri` + `src/`: Tauri 2 with a Svelte 5 interface

## Running and building

Requirements: Rust (stable), Node.js ≥ 20, on Linux additionally WebKitGTK 4.1
(`webkit2gtk-4.1`) and GStreamer plugins for MP3/Ogg playback.

```bash
npm install
npx tauri dev                       # development mode
npx tauri dev -- -- /path/Gw2.dat   # open an archive directly
npx tauri build                     # release build plus installer packages
```

The app reopens the last archive at startup. A `.dat` can also be dropped onto
the window.

### Portable Windows build

The app needs no installer: the release executable contains the whole
frontend, and Tauri links the Visual C++ runtime in statically (the Universal
CRT it uses is part of Windows). Do not add `-C target-feature=+crt-static`:
it conflicts with Tauri's static runtime and the link fails with undefined
C functions (`strlen`, `pow`, …). Build only the executable, on Windows:

```bash
npx tauri build --no-bundle
```

or cross-compiled from Linux (needs `cargo-xwin`, `lld` and `llvm`, and
`rustup target add x86_64-pc-windows-msvc`):

```bash
npx tauri build --no-bundle --runner cargo-xwin --target x86_64-pc-windows-msvc
```

Zip `gw2browser.exe` (from `target/release/` or
`target/x86_64-pc-windows-msvc/release/`); it runs on Windows 10 and 11, which
ship the WebView2 runtime. Settings, index caches and the web view's data are
kept in a `data` folder next to the executable (on every platform), so deleting the unpacked folder
removes everything. Only when that folder cannot be written does the app use
the per-user folders (`%APPDATA%`/`%LOCALAPPDATA%`, `~/.config`/`~/.cache`).

## Features

| Area | Preview | Export (converted) |
|---|---|---|
| Textures (ATEX/ATTX/ATEP/ATEU/ATEC/ATET/CTEU: DXT1–5, DXTA, DXTL, DXTN, 3DCX, BC7) | Zoom, pan, channels R/G/B/A, backgrounds | PNG |
| DDS (DXT, RGB16/24/32, L8, V8U8, R32F), JPEG, PNG, WebP | as above | PNG |
| Sounds (PF ASND MP3/Ogg, asnd, MP3 (also without ID3 tag), Ogg) | Player | MP3/Ogg |
| Sound banks (ABNK) | List of all entries, play and export individually | one file per playable entry |
| Sound bank indexes (ABIX) | Which bank file holds each bank number per voice language; pick a bank and play its sounds right there (banks of voice languages that are not installed are marked) | CSV |
| Audio scripts (AMSP, 21,149 files) | Sound events with their candidate files (playable), volume/pitch ranges, fades, looping and playback settings; trigger keys (cues at animation times), handlers with byte code, script references, properties; the global audio settings script lists all 1,816 music playlists with playable tracks | TXT |
| Models (MODL, all chunk versions) | 3D view with textures, wireframe, skeleton; **animation playback** (own clips, the skeleton's clips and every animation set made for the skeleton) with timeline and speed | OBJ + MTL + PNG textures |
| Animation clips (MODL with only an ANIM chunk, 15,218 files) | Name, duration, frames/fps, track groups | raw |
| Strings (strs) | Table with filter | TSV |
| EULA | Text per language | TXT |
| Game content (cntc) | XML with syntax highlighting | XML |
| Bitmap fonts (AFNT) | Glyph atlas per font and character range | PNG per font and range |
| Glyph files of bitmap fonts | Glyph atlas with character range and the fonts using it | PNG |
| Fonts: TrueType (10 files), WOFF and EOT (1 each, under Fonts) | Specimen in the font itself: sample text in several sizes, every mapped character; WOFF is unpacked to TrueType; the EOT font is compressed with MicroType Express, so only the names from its header are shown | TTF (TrueType and WOFF; EOT only raw) |
| Star fields (STAR, 2 files) | Sky chart (dome or panorama), stars sized by brightness, colouring by the unknown values, values on hover | CSV |
| Animation sequences (anic) | Searchable table of 1,853 sequences / 8,858 variants: steps, movement, blend times, timeline with triggers, charge stages | JSON |
| Emote animations (emoc, 1 file, 314 emotes) | Searchable emote list; per emote its variants (one per body type, mount or prop) with the animation file (jump to it), intro/loop/outro timeline, blend times, start offset; the selected variant's model in 3D or its summary | CSV |
| Model collision manifest (cmaC, 1 file) | Searchable list of 9,188 models with collision: original asset path from development, model and collision file (jump to them) | CSV |
| Portal manifests (prlt, 89 files) | Maps by GUID with their start points (position, radius, models by type) and exits; the models loaded for a start point, filterable by type, with links | CSV |
| Collision (hvkC, 9,188 files) | 3D view of the Havok collision meshes and blockers in model units, per collision state (animation sequence), with the model it belongs to overlaid | OBJ |
| Map shadows (mpsd, 279 files) | Shadow map image with tile counts and sun position; detailed tiles are shown at the middle of their value range (their per-pixel encoding is not decoded) | PNG |
| Map metadata (mMet, 1 file) | All 1,244 map ids with their map type, filterable | CSV |
| Character composition (cmpc, 1 file) | Per race and sex (29): skeleton, colour palettes, body shapes, faces, hair styles, beards, ears, skin patterns, animation overrides; every body and armour part with mesh, texture, normal map and dye masks (mesh in 3D and texture side by side); NPC preset variants with their parts and colour adjustments; armour texture atlas layouts | raw |
| Cinematics (CINP, 1,457 files; scene versions 35 and 36, 938 files, are decoded) | Timeline per sequence with a row per track group (camera, model, image, text, audio, light, filter, …) and its keys and triggers; track curves of a selected group; the scene's dialogue (English subtitles) with playable voice lines; files, scripts, ambient lights, species | TXT (sequences and dialogue; older versions raw) |
| Maps (mapc, 287 files) | 3D view: terrain heightmap coloured by height and slope, sea level and lakes, props as textured models (loaded within a draw distance around the focus point with markers for the rest, or all at once); click a prop for its model id; size, prop and chunk overview | 16-bit heightmap PNG |
| Materials (AMAT, 2,854 files) | Texture slots, techniques with their passes and effects (render state decoded: blending, depth, culling), sampler states; per shader: uniforms with registers, input/output signatures, vertex attributes, texture bindings and a shader model 5 disassembly annotated with uniform names | TXT (everything incl. disassembly) |
| World map tiles (PIMG, 3,298 files, under Maps) | Pan and zoom like a map: an overview of the whole layer, sharper pages load as you zoom in (up to the full 512×512 per page); layers, page and texture under the cursor | PNG of a layer at a chosen pixels-per-page |
| TextPack (txtm manifest, txtV variants, txtv voices) | Every game text by ID and language, full-text search, jump to ID; text variants side by side; voice lines with playback from the sound banks (only playable by default) | raw |
| Text files (2,696, sorted by content: HTML, JavaScript, documentation, CSS, JSON, plain text, PHP, XML, licenses, ActionScript; mostly the YUI 3 web library the game ships for its web UI) | Text view with syntax highlighting (HTML/XML with embedded JS and CSS, JavaScript, CSS, JSON, PHP, Markdown, ActionScript; highlighting for files up to 1 MiB, the first 4 million characters are shown) | raw |
| Bink 2 videos (KB2, 153 files) | Resolution, frame rate, frame count, duration, audio tracks, bit rate. **Not playable**: Bink 2 is RAD Game Tools' proprietary codec without an open decoder, so the preview says so and points to RAD Video Tools | raw (`.bk2`) |
| Web browser: resource packs (Chromium `.pak` v5, 4 files) | Every resource by id and content kind (PNG/WebP/SVG images, HTML, JavaScript, CSS, JSON, text, fonts); gzip and Chromium's brotli compression are unpacked; images, highlighted text or hex per resource | raw |
| Web browser: ICU data (`icudt71l`, 1 file) | Copyright, versions, the 2,124 entries grouped by tree (locales, collation, break rules, …) with sizes | raw |
| Web browser: V8 snapshots (2 files) | V8 version (10.3.174.18, i.e. Chromium 103), contexts, checksum | raw |
| Web browser: Flash movies (SWF, 2 files) | SWF version, compression, stage size, frame rate and count (not played: Flash Player is discontinued) | raw |
| Everything else | Hex view | raw |

Every file can also be exported raw (decompressed). Whole categories or search
results are exported in parallel with a progress bar.

Tree and file list show an icon for each of the 73 file types; a category
takes the icon of its content (of its most common kind if it mixes types).

Models are sorted into two subfolders by what they contain: **Meshes** for
model files with geometry or materials, **Animation** for skeletons, clips and
other rig data (only the file up to the skeleton, animation, material and mesh
chunk headers is read while scanning):

| Folder | Files | Contents |
|---|---|---|
| Meshes / Models with animation | 57,552 | meshes, and animations to play: its own clips, its skeleton file's clips or animation sets made for its skeleton (the same sources the 3D viewer lists) |
| Meshes / Models without animation | 119,563 | meshes, nothing to animate them with |
| Meshes / Only materials | 344 | materials, no meshes, skeleton or animations |
| Meshes / Empty | 30 | neither meshes, materials nor animations, only a skeleton reference |
| Animation / Skeletons | 5,951 | no meshes, own skeleton; other models are bound to these |
| Animation / Animations | 15,218 | a single ANIM chunk (Granny animation clip) |
| Animation / Animation sets | 1,514 | no meshes, animations for another file's skeleton |
| Animation / Bone scales | 64 | `bone` files: body proportions for character creation |
| Animation / Animation sequences | 1 | `anic`: how game actions play their animations (see above) |
| Animation / Emote animations | 1 | `emoc`: animation files and timings of the 314 emotes (see above) |

Search: file ID (base or file ID, also by prefix), `#<MFT index>`, or part of
an asset name or path. The 9,188 models with collision and their collision files
show their original asset name from development (e.g. `Crab.amdl2`, read from
the model collision manifest); when one of them is exported on its own, the
suggested file name is `<asset>_<id>.<ext>`.
Shortcuts: `Ctrl+O` open, `Ctrl+F` search, arrow keys/PageUp/PageDown/Home/End
in the file list.

## Differences from the original

### Speed
- **Positional reads** instead of seek+read; thread-safe and robust against the
  file changing underneath (no `mmap` and therefore no SIGBUS during game
  updates).
- **Scanning reads only the start of each file** (the original reads every
  compressed file completely) and runs in parallel on all cores, off the UI
  thread. 818,000 files: ~0.5 s with a warm page cache, about a minute cold
  over NTFS/FUSE.
- **Incremental index cache**: after a game patch only changed MFT entries are
  scanned again. Loading the cache takes ~40 ms.
- ID lookup via hash map (the original searches linearly and freezes).

### Bugs fixed (verified against a real `Gw2.dat`)
- **Uncompressed files**: the 4-byte checksum at the end of the *last* 64 KiB
  block was not removed (4 bytes of garbage at the end of every such file);
  the read function also returned `sizeof(pointer)` instead of the size read.
- **64-bit PF files**: files with header flag `0x4` use 8-byte offsets. gw2formats
  only knows 32-bit offsets, which affected ~100,000 models, ~36,000 sound banks,
  18,667 sounds and others. Those sounds were listed as "unknown" in the original.
- **Models**: all `MODL` versions v65–v70 and both pointer widths. The original
  only reads v65 with 32-bit offsets (hence the crashes).
- **DXTL textures**: gw2dattools reads only half of each block (plus undefined
  stack memory via an oversized `memcpy`). DXTL is laid out like DXT5; the colour
  is multiplied by the luminance stored in alpha and the result is opaque.
- **Textures whose size is not a multiple of four** (e.g. 66×66, 74×74) are
  decoded with the correct block stride instead of producing garbage blocks.
- BC7 textures (~18,500) are supported.
- Additional DDS formats: RGB16/24, V8U8, R32F.
- Bounds checks everywhere: damaged data leads to an error message, not a crash.
- Bounding boxes of models (race condition, wrong initial value) and the UV set
  selection are fixed.
- Model files without geometry (the original reports "no data"/crashes) are
  recognised as what they are: Granny animation clips. They have their own
  category "Models / Animation / Animations" with name, duration and frame rate.
- Animations: Granny 3D clips are decoded (all 18 curve formats in use,
  including compressed quaternions) and played on the model's skeleton with
  skinning. Mesh bones are matched to skeleton bones by their name tokens.
- Bitmap fonts: all 13 character ranges instead of Basic Latin only; glyph files
  are recognised through the font's references instead of a hard-coded ID list.
- Sound banks: Ogg entries as well as MP3.
- Paged images: the original listed them only. Pages are ATEP textures or
  solid colours (stored as BGRA) on a grid; the world map has 80×112 pages
  (40,960×57,344 pixels).
- Audio scripts: the original listed them only. gw2formats describes chunk
  version 30; version 33 (all but eight files, which use versions 28–30) adds
  one dword after `channelMax` in each meta sound. All 133,129 meta sounds
  decode and all 568,894 sound file references resolve.
- Materials: the original listed them only. The `BGFX` chunk holds
  [bgfx](https://github.com/bkaradzic/bgfx) shader binaries (GW2's DirectX 11
  renderer is built on bgfx) wrapping DXBC bytecode; gw2formats only describes
  chunk version 1 and with a wrong pass layout (effects are stored inline, not as
  a pointer list). All 2,854 files and their 73,655 distinct shaders decode and
  disassemble. Names (texture slots, effects) are 5-bit letter tokens like bone
  names; render and sampler states use bgfx's state bits.
- Maps: the original's map reader stopped after locating the heightmap. Terrain
  is now assembled from its chunks (32 or 64 tiles per chunk, three overlapping
  samples per edge, rows stored from maximum to minimum y, holes marked with 10000),
  props are placed with their Direct3D yaw-pitch-roll rotation, and water is read.

## Architecture

```
crates/gw2dat/            core library
  src/archive.rs          header, MFT, ID table, read/peek
  src/inflate/            Huffman/LZ decompression (.dat) and texture decompression
  src/filetype.rs         type detection and categories (table-driven)
  src/index.rs            parallel scan, incremental cache
  src/formats/            pf (PF container), image, audio, audio_script, cinematic, collision, composite, emotes, manifests, model, skeleton, animation, granny (Granny clips), sequences, map, map_data, material, paged_image, shader, stars, text, textpack, font, truetype (TrueType, WOFF, EOT), content, media (Bink 2, SWF headers), web (Chromium PAK, ICU, V8)
  examples/               developer tools (see below)
  tests/real_archive.rs   tests against a real Gw2.dat
src-tauri/                Tauri backend (commands, export)
src/                      Svelte frontend
```

### Developer tools

```bash
cargo run --release -p gw2dat --example validate -- res/Gw2.dat      # decode every file, statistics
cargo run --release -p gw2dat --example scan -- res/Gw2.dat          # scan/cache timing
cargo run --release -p gw2dat --example inspect -- res/Gw2.dat <id>  # MFT, PF chunks, hex
cargo run --release -p gw2dat --example extract -- res/Gw2.dat <id> [--raw] [out]
cargo run --release -p gw2dat --example render -- res/Gw2.dat <outdir> <model id>...
```

`validate` is the quickest check after a game update: at the time of writing,
818,082 files pass with 0 errors (text and glyph files are only counted, plain
binaries skipped).

### Tests

```bash
cargo test -p gw2dat                                                      # unit tests
GW2_DAT=$PWD/res/Gw2.dat cargo test --release -p gw2dat --test real_archive
```

## Known limitations

- The archive only contains the sound banks of the game's voice language; the
  sound bank index also lists the banks of the other voice languages.
- Encrypted content (most sound bank entries, some strings) cannot be read.
  The keys come from the game server. This includes the dialogue texts: of
  256,169 voiced texts only one is readable, and 207,236 of 1.2 million English
  texts are readable in total.
- The first time a TextPack voices file is opened, all sound banks are scanned
  for voice IDs (~20 s); the result is cached next to the index cache.
- Cinematic scenes older than version 35 (519 files) lay out their data
  differently and are not decoded.
- Skeletons are only summarised. Detailed map shadow tiles are decompressed,
  but how their 64×64 16-bit samples are encoded is not known yet.
- The map view colours the terrain by height; the terrain's own texture
  layers (`trni`, paged images) and lights, decals and zones are not drawn yet.
- Bink 2 videos cannot be played: the codec is proprietary to RAD Game Tools
  and there is no open decoder. Export them raw and play them with RAD Video
  Tools.
- The EOT font is compressed with MicroType Express, which is not decoded.
  Flash movies are not played.
- The 3D view shows diffuse textures only. Materials (AMAT shaders) are not
  replicated.

## License

GPL-3.0-or-later. Derived from Gw2Browser (Khralkatorrix, Rhoot and others),
gw2DatTools (Antoine Hom) and gw2formats (Rhoot, Khralkatorrix). See the
original project for details. Guild Wars 2 © ArenaNet, LLC.
