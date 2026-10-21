//! Map metadata (PF `mMet`) and map shadows (PF `mpsd`).
//!
//! - Map metadata lists every map id with a map type number.
//! - A shadow map is the sun's shadow over a map, seen from the light
//!   (`shadow_eye`, `shadow_dir`), as a grid of 64×64 tiles of light values
//!   (1 = fully lit). A tile is either one value for all its pixels or
//!   compressed detail: ArenaNet's Huffman/LZ stream holding the tile's value
//!   range and 64×64 16-bit samples. The encoding of those samples is not
//!   known yet, so detail tiles are shown at the middle of their range.

use super::image::RgbaImage;
use super::pf::PackFile;
use crate::error::{Error, Result};
use crate::fourcc::fcc;

pub const MMET: u32 = fcc(b"mMet");
pub const MPSD: u32 = fcc(b"mpsd");

/// (map id, map type).
pub fn metadata(data: &[u8]) -> Result<Vec<(u16, u8)>> {
    let file = PackFile::parse(data)?;
    if file.file_type != MMET {
        return Err(Error::format("map metadata", "not an mMet file"));
    }
    let mut c = file.require_chunk(fcc(b"Main"))?.cursor();
    let (count, mut item) = c.array()?;
    (0..count).map(|_| Ok((item.u16()?, item.u8()?))).collect()
}

#[derive(Clone, Debug)]
pub struct Shadow {
    pub width: u32,
    pub height: u32,
    pub tile_width: u32,
    pub tile_height: u32,
    /// Row-major tiles.
    pub tiles: Vec<ShadowTile>,
    pub world_to_shadow: [f32; 16],
    pub s: [f32; 2],
    pub t: [f32; 2],
    pub u: [f32; 2],
    pub eye: [f32; 3],
    pub direction: [f32; 3],
    pub up: [f32; 3],
}

#[derive(Clone, Debug)]
pub enum ShadowTile {
    /// One light value for the whole tile.
    Uniform(f32),
    /// Compressed detail with its value range.
    Detail {
        min: f32,
        max: f32,
    },
    Unknown(u32),
}

pub fn shadow(data: &[u8]) -> Result<Shadow> {
    let file = PackFile::parse(data)?;
    if file.file_type != MPSD {
        return Err(Error::format("map shadow", "not an mpsd file"));
    }
    let mut c = file.require_chunk(fcc(b"shad"))?.cursor();
    let (width, height, tile_width, tile_height) = (c.u32()?, c.u32()?, c.u32()?, c.u32()?);
    let (count, mut item) = c.array()?;
    let mut tiles = Vec::with_capacity(count.min(1 << 20));
    let detail_size = 8 + tile_width as usize * tile_height as usize * 2;
    for _ in 0..count {
        let mode = item.u32()?;
        let bytes = item.byte_array()?;
        let float = |b: &[u8]| f32::from_le_bytes(b[..4].try_into().unwrap());
        tiles.push(match mode {
            0 if bytes.len() >= 4 => ShadowTile::Uniform(float(bytes)),
            2 => {
                let head = crate::inflate::inflate_raw(bytes, detail_size.min(8))?;
                ShadowTile::Detail { min: float(&head), max: float(&head[4..]) }
            }
            other => ShadowTile::Unknown(other),
        });
    }
    let mut floats = |n: usize| (0..n).map(|_| c.f32()).collect::<Result<Vec<f32>>>();
    let world_to_shadow: [f32; 16] = floats(16)?.try_into().unwrap();
    let st = floats(6)?;
    let vectors = floats(9)?;
    Ok(Shadow {
        width,
        height,
        tile_width,
        tile_height,
        tiles,
        world_to_shadow,
        s: [st[0], st[1]],
        t: [st[2], st[3]],
        u: [st[4], st[5]],
        eye: [vectors[0], vectors[1], vectors[2]],
        direction: [vectors[3], vectors[4], vectors[5]],
        up: [vectors[6], vectors[7], vectors[8]],
    })
}

/// The shadow map as a grayscale image (white = lit); detail tiles at the
/// middle of their range.
pub fn shadow_image(shadow: &Shadow) -> RgbaImage {
    let (w, h) = (shadow.width as usize, shadow.height as usize);
    let (tw, th) = (shadow.tile_width.max(1) as usize, shadow.tile_height.max(1) as usize);
    let columns = w.div_ceil(tw);
    let mut pixels = vec![0u8; w * h * 4];
    for (i, tile) in shadow.tiles.iter().enumerate() {
        let value = match *tile {
            ShadowTile::Uniform(v) => v,
            ShadowTile::Detail { min, max } => (min + max) / 2.0,
            ShadowTile::Unknown(_) => continue,
        };
        let level = (value.clamp(0.0, 1.0) * 255.0).round() as u8;
        let (x0, y0) = ((i % columns) * tw, (i / columns) * th);
        for y in y0..(y0 + th).min(h) {
            for x in x0..(x0 + tw).min(w) {
                let at = (y * w + x) * 4;
                pixels[at..at + 4].copy_from_slice(&[level, level, level, 255]);
            }
        }
    }
    RgbaImage { width: shadow.width, height: shadow.height, pixels, format: "map shadow".into() }
}
