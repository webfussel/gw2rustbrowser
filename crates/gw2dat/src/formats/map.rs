//! Maps (PF `mapc`): terrain, placed props and map collision.
//!
//! A map file holds one chunk per subsystem. The preview uses three of them:
//! - `parm`: the map rectangle in world units,
//! - `trn\0`: the terrain heightmap, stored per terrain chunk,
//! - `prp2`: props, i.e. model files placed with position, rotation and scale.
//!
//! World coordinates are DirectX style: x and y span the ground plane, z points
//! down (heights are negative above the zero level).

use super::pf::{Cursor, PackFile};
use crate::error::{Error, Result};
use crate::fourcc::{display, fcc};

pub const MAPC: u32 = fcc(b"mapc");
const PARM: u32 = fcc(b"parm");
const TERRAIN: u32 = fcc(b"trn\0");
const PROPS: u32 = fcc(b"prp2");
const WATER: u32 = fcc(b"watr");

#[derive(Clone, Debug)]
pub struct Map {
    /// `[min x, min y, max x, max y]` in world units.
    pub rect: [f32; 4],
    pub terrain: Option<Terrain>,
    pub props: Vec<Prop>,
    pub water: Option<Water>,
    /// Every chunk of the file: name, version, size in bytes.
    pub chunks: Vec<(String, u16, usize)>,
}

/// Height samples on a regular grid covering `Map::rect`.
#[derive(Clone, Debug)]
pub struct Terrain {
    /// Samples per row and number of rows.
    pub columns: usize,
    pub rows: usize,
    /// Row-major world z values; row 0 lies at the map's minimum y.
    pub heights: Vec<f32>,
}

/// Water: a plane covering the whole map and/or polygons at their own level.
#[derive(Clone, Debug)]
pub struct Water {
    pub flags: u32,
    /// World z of the map-wide water plane.
    pub plane_z: f32,
    pub surfaces: Vec<WaterSurface>,
}

#[derive(Clone, Debug)]
pub struct WaterSurface {
    pub flags: u32,
    pub z: f32,
    /// Outline in the ground plane.
    pub outline: Vec<[f32; 2]>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PropKind {
    Static,
    Animated,
    /// One of the copies of an instanced prop.
    Instance,
    /// Part of a group of props.
    Meta,
}

#[derive(Clone, Debug)]
pub struct Prop {
    pub model: Option<u32>,
    pub position: [f32; 3],
    /// Euler angles in radians around x, y and z.
    pub rotation: [f32; 3],
    pub scale: f32,
    pub kind: PropKind,
}

pub fn decode(data: &[u8]) -> Result<Map> {
    let file = PackFile::parse(data)?;
    if file.file_type != MAPC {
        return Err(Error::format("map", "not a mapc file"));
    }
    let chunks =
        file.chunks().map(|c| (display(c.magic).trim_end_matches('.').to_owned(), c.version, c.size)).collect();
    let mut parm = file.require_chunk(PARM)?.cursor();
    let rect = [parm.f32()?, parm.f32()?, parm.f32()?, parm.f32()?];
    let terrain = match file.chunk(TERRAIN) {
        Some(chunk) => terrain(chunk.cursor())?,
        None => None,
    };
    let props = match file.chunk(PROPS) {
        Some(chunk) if chunk.version == 21 => props(chunk.cursor())?,
        Some(chunk) => return Err(Error::format("map", format!("unsupported prop chunk version {}", chunk.version))),
        None => Vec::new(),
    };
    let water = match file.chunk(WATER) {
        Some(chunk) if chunk.version == 1 => Some(water(chunk.cursor())?),
        _ => None,
    };
    Ok(Map { rect, terrain, props, water, chunks })
}

/// `PackMapTerrainV14`/`V15` up to the height map (both versions share it).
fn terrain(mut c: Cursor) -> Result<Option<Terrain>> {
    let (tiles_x, tiles_y) = (c.u32()? as usize, c.u32()? as usize);
    let _swap_distance = c.f32()?;
    let (count, mut samples) = c.array()?;
    if tiles_x == 0 || tiles_y == 0 || count == 0 {
        return Ok(None);
    }
    // Terrain chunks are 32×32 tiles, in newer maps 64×64, and store
    // `tiles + 3` samples per edge: the last three repeat the first samples of
    // the neighbouring chunk.
    let layout = [32, 64].into_iter().find_map(|tiles: usize| {
        let (chunks_x, chunks_y) = (tiles_x.div_ceil(tiles), tiles_y.div_ceil(tiles));
        let edge = tiles + 3;
        (count == chunks_x * chunks_y * edge * edge).then_some((tiles, edge, chunks_x, chunks_y))
    });
    let Some((tiles, edge, chunks_x, chunks_y)) = layout else {
        return Err(Error::format("map", format!("{count} height samples for {tiles_x}×{tiles_y} tiles")));
    };
    let stored: Vec<f32> = samples.take(count * 4)?.as_chunks::<4>().0.iter().map(|b| f32::from_le_bytes(*b)).collect();
    let (columns, rows) = (chunks_x * tiles + 1, chunks_y * tiles + 1);
    // Global sample index -> (chunk, sample in chunk); the last row/column
    // comes from the last chunk's first overlapping sample.
    let split =
        |i: usize, chunks: usize| if i == chunks * tiles { (chunks - 1, tiles) } else { (i / tiles, i % tiles) };
    let mut heights = Vec::with_capacity(columns * rows);
    // The stored rows run from the map's maximum y down to its minimum.
    for y in (0..rows).rev() {
        let (chunk_y, local_y) = split(y, chunks_y);
        for x in 0..columns {
            let (chunk_x, local_x) = split(x, chunks_x);
            heights.push(stored[(chunk_y * chunks_x + chunk_x) * edge * edge + local_y * edge + local_x]);
        }
    }
    Ok(Some(Terrain { columns, rows, heights }))
}

/// `PackMapWaterV1`.
fn water(mut c: Cursor) -> Result<Water> {
    let flags = c.u32()?;
    let plane_z = c.f32()?;
    let (count, mut item) = c.array()?;
    let mut surfaces = Vec::with_capacity(count.min(1 << 16));
    for _ in 0..count {
        item.skip(8)?; // guid
        let flags = item.u32()?;
        let z = item.f32()?;
        let (points, mut p) = item.array()?;
        let outline = (0..points).map(|_| Ok([p.f32()?, p.f32()?])).collect::<Result<_>>()?;
        surfaces.push(WaterSurface { flags, z, outline });
    }
    Ok(Water { flags, plane_z, surfaces })
}

/// Fields shared by every placed prop (`PackMapPropObjV21` and relatives).
fn prop_object(c: &mut Cursor, kind: PropKind) -> Result<Prop> {
    let model = c.file_ref()?;
    c.array()?; // blit textures
    c.array()?; // shader constants
    c.skip(8 + 8 + 16)?; // guid, permutation, bounds
    let position = [c.f32()?, c.f32()?, c.f32()?];
    let rotation = [c.f32()?, c.f32()?, c.f32()?];
    c.skip(4)?; // colour
    let scale = c.f32()?;
    c.skip(4 + 4 + 4 + 4 + 2 + 2 + 1 + 1)?; // lods, flags, reserved, broad/bucket id, byte data, sort layer
    Ok(Prop { model, position, rotation, scale, kind })
}

/// `PackMapPropV21`.
fn props(mut c: Cursor) -> Result<Vec<Prop>> {
    let mut props = Vec::new();
    let (count, mut item) = c.array()?;
    for _ in 0..count {
        props.push(prop_object(&mut item, PropKind::Static)?);
    }
    let (count, mut item) = c.array()?;
    for _ in 0..count {
        props.push(prop_object(&mut item, PropKind::Animated)?);
        item.skip(8)?; // animation sequence
    }
    let (count, mut item) = c.array()?;
    for _ in 0..count {
        let base = prop_object(&mut item, PropKind::Instance)?;
        let (transforms, mut t) = item.array()?;
        item.array()?; // original guids
        for _ in 0..transforms {
            let position = [t.f32()?, t.f32()?, t.f32()?];
            let rotation = [t.f32()?, t.f32()?, t.f32()?];
            let scale = t.f32()?;
            props.push(Prop { position, rotation, scale, ..base.clone() });
        }
    }
    let (count, mut item) = c.array()?; // tool objects: grouping only
    for _ in 0..count {
        item.skip(8 + 4 + 1)?;
        item.array()?;
    }
    let (count, mut item) = c.array()?;
    for _ in 0..count {
        props.push(prop_object(&mut item, PropKind::Meta)?);
        item.skip(4 + 1 + 8 + 12)?; // layer mask, glom type, parent, glom origin
    }
    props.retain(|p| p.position.iter().chain(&p.rotation).all(|v| v.is_finite()) && p.scale.is_finite());
    Ok(props)
}

/// World z that marks a hole in the terrain.
pub const TERRAIN_HOLE: f32 = 10000.0;

impl Terrain {
    /// Lowest and highest world z, ignoring holes.
    pub fn z_range(&self) -> Option<(f32, f32)> {
        self.heights.iter().filter(|&&h| h != TERRAIN_HOLE).fold(None, |range, &h| match range {
            None => Some((h, h)),
            Some((lo, hi)) => Some((f32::min(lo, h), f32::max(hi, h))),
        })
    }
}

/// The terrain as a 16-bit grayscale PNG: white is high, maximum y
/// at the top, holes black.
pub fn heightmap_png(map: &Map) -> Result<Vec<u8>> {
    let terrain = map.terrain.as_ref().ok_or_else(|| Error::format("map", "the map has no terrain"))?;
    let (lo, hi) = terrain.z_range().unwrap_or((0.0, 0.0));
    let span = (hi - lo).max(1.0);
    let mut pixels = Vec::with_capacity(terrain.heights.len() * 2);
    for row in terrain.heights.chunks_exact(terrain.columns).rev() {
        for &h in row {
            // z points down: the smallest z is the highest point.
            let value = if h == TERRAIN_HOLE { 0 } else { 1 + ((hi - h) / span * 65534.0).round() as u16 };
            pixels.extend_from_slice(&value.to_ne_bytes());
        }
    }
    let mut out = Vec::new();
    let encoder = image::codecs::png::PngEncoder::new(&mut out);
    image::ImageEncoder::write_image(
        encoder,
        &pixels,
        terrain.columns as u32,
        terrain.rows as u32,
        image::ExtendedColorType::L16,
    )
    .map_err(|e| Error::format("PNG", e.to_string()))?;
    Ok(out)
}
