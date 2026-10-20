# GW2 Browser (Rust)

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

## Features

| Area | Preview | Export (converted) |
|---|---|---|
| Textures (ATEX/ATTX/ATEP/ATEU/ATEC/ATET: DXT1–5, DXTA, DXTL, DXTN, 3DCX, BC7) | Zoom, pan, channels R/G/B/A, backgrounds | PNG |
| DDS (DXT, RGB16/24/32, L8, V8U8, R32F), JPEG, PNG, WebP | as above | PNG |
| Sounds (PF ASND MP3/Ogg, asnd, MP3, Ogg) | Player | MP3/Ogg |
| Sound banks (ABNK) | List of all entries, play and export individually | one file per playable entry |
| Models (MODL, all chunk versions) | 3D view with textures, wireframe | OBJ + MTL + PNG textures |
| Animation clips (MODL with only an ANIM chunk, 15,218 files) | Name, duration, frames/fps, track groups | raw |
| Strings (strs) | Table with filter | TSV |
| EULA | Text per language | TXT |
| Game content (cntc) | XML | XML |
| Bitmap fonts (AFNT) | Glyph atlas per font and character range | PNG per font and range |
| Text, everything else | Text or hex view | raw |

Every file can also be exported raw (decompressed). Whole categories or search
results are exported in parallel with a progress bar.

Tree and file list show an icon for each of the 67 file types; a category
takes the icon of its content (of its most common kind if it mixes types).

Models are sorted by what they are, judged from their skeleton, animation,
material and mesh data (only the file up to those chunk headers is read while
scanning, not the geometry):

| Folder | Files | Detection |
|---|---|---|
| Props | 92,354 | single bone: rocks, terrain pieces, buildings |
| Equipment & body parts | 40,092 | bound to a player skeleton (one shared by ≥ 1,000 models), or untextured on another skeleton: armor, hair, faces |
| Multi-part props | 24,473 | own multi-bone skeleton, no animations |
| Animated | 18,609 | own skeleton with an animation bank: cloth, banners, moving objects |
| Creatures & NPCs | 1,587 | textured and bound to a non-player skeleton: animals, monsters, NPCs |
| Animations | 15,218 | a single ANIM chunk (Granny animation clip) |
| Skeletons | 5,951 | no meshes, own skeleton; the other models are bound to these |
| Animation sets | 1,514 | no meshes, animations for another file's skeleton |
| Without geometry | 374 | neither meshes nor animations |
| Bone scales | 64 | `bone` files: body proportions for character creation |

The eight player skeletons are recognised by counting how many models share
each skeleton, so no id list has to be maintained.

Search: file ID (base or file ID, also by prefix) or `#<MFT index>`.
Shortcuts: `Ctrl+O` open, `Ctrl+F` search, arrow keys/PageUp/PageDown in the
file list.

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
  scanned again. Loading the cache takes ~30 ms.
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
  category "Models / Animations" with name, duration and frame rate.
- Bitmap fonts: all 13 character ranges instead of Basic Latin only; glyph files
  are recognised through the font's references instead of a hard-coded ID list.
- Sound banks: Ogg entries as well as MP3.

## Architecture

```
crates/gw2dat/            core library
  src/archive.rs          header, MFT, ID table, read/peek
  src/inflate/            Huffman/LZ decompression (.dat) and texture decompression
  src/filetype.rs         type detection and categories (table-driven)
  src/index.rs            parallel scan, incremental cache
  src/formats/            pf (PF container), image, audio, model, text, font, content
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
778,936 files decode with 0 errors.

### Tests

```bash
cargo test -p gw2dat                                                      # unit tests
GW2_DAT=$PWD/res/Gw2.dat cargo test --release -p gw2dat --test real_archive
```

## Known limitations

- Encrypted content (most sound bank entries, some strings) cannot be read.
  The keys come from the game server.
- Map files, animations, skeletons and collision data are only listed and
  exported raw. The original could not display them either.
- The 3D view shows diffuse textures only. Materials (AMAT shaders) are not
  replicated.

## License

GPL-3.0-or-later. Derived from Gw2Browser (Khralkatorrix, Rhoot and others),
gw2DatTools (Antoine Hom) and gw2formats (Rhoot, Khralkatorrix). See the
original project for details. Guild Wars 2 © ArenaNet, LLC.
