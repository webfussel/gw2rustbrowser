//! Havok collision data (PF `hvkC`, chunk `havk` version 16; maps carry the
//! same chunk).
//!
//! A collision file holds triangle meshes ("collisions"), blockers (vertex
//! fences), animation states that switch meshes on and off (e.g. a door that
//! opens), and geometries that group those states. Coordinates are Havok
//! metres with z up: a model's own space is `(x, y, -z) * 32`.

use super::pf::{Cursor, PackFile};
use crate::error::{Error, Result};
use crate::fourcc::fcc;

pub const HVKC: u32 = fcc(b"hvkC");
const HAVK: u32 = fcc(b"havk");

/// Model units per Havok unit.
pub const HAVOK_SCALE: f32 = 32.0;

#[derive(Clone, Debug)]
pub struct Collision {
    pub bounds: [[f32; 3]; 2],
    pub flags: u32,
    pub water_z: f32,
    pub meshes: Vec<CollisionMesh>,
    /// Blocker fences as vertex lists.
    pub blockers: Vec<Vec<[f32; 3]>>,
    pub animations: Vec<CollisionAnimation>,
    pub geometries: Vec<Geometry>,
    pub obstacle_models: usize,
    pub prop_models: usize,
    pub zone_models: usize,
    pub water_volumes: usize,
}

#[derive(Clone, Debug)]
pub struct CollisionMesh {
    pub vertices: Vec<[f32; 3]>,
    /// Three per triangle.
    pub indices: Vec<u16>,
    /// Surface (material) per triangle.
    pub surfaces: Vec<u16>,
}

/// A state of the collision: which meshes and blockers are active.
#[derive(Clone, Debug)]
pub struct CollisionAnimation {
    pub sequence: u64,
    pub meshes: Vec<u32>,
    pub blockers: Vec<u32>,
}

#[derive(Clone, Debug)]
pub struct Geometry {
    pub quantized_extents: u8,
    pub animations: Vec<u32>,
    pub nav_mesh: u16,
}

pub fn decode(data: &[u8]) -> Result<Collision> {
    let file = PackFile::parse(data)?;
    if file.file_type != HVKC {
        return Err(Error::format("collision", "not an hvkC file"));
    }
    let chunk = file.require_chunk(HAVK)?;
    if chunk.version != 16 {
        return Err(Error::Unsupported(format!("havk chunk version {}", chunk.version)));
    }
    decode_chunk(chunk.cursor())
}

/// Reads a `havk` chunk (`PackMapCollideV16`).
pub fn decode_chunk(mut c: Cursor) -> Result<Collision> {
    let bounds = [vec3(&mut c)?, vec3(&mut c)?];
    let flags = c.u32()?;
    let water_z = c.f32()?;
    let meshes = list(&mut c, |m| {
        let indices = list(m, |i| i.u16())?;
        let vertices = list(m, vec3)?;
        let surfaces = list(m, |s| s.u16())?;
        m.array()?; // MOPP code
        if indices.len() % 3 != 0 || indices.iter().any(|&i| i as usize >= vertices.len()) {
            return Err(Error::format("collision", "triangle indices outside the mesh"));
        }
        Ok(CollisionMesh { vertices, indices, surfaces })
    })?;
    let blockers = list(&mut c, |b| list(b, vec3))?;
    let animations = list(&mut c, |a| {
        Ok(CollisionAnimation { sequence: a.u64()?, meshes: list(a, |i| i.u32())?, blockers: list(a, |i| i.u32())? })
    })?;
    let geometries = list(&mut c, |g| {
        Ok(Geometry { quantized_extents: g.u8()?, animations: list(g, |i| i.u32())?, nav_mesh: g.u16()? })
    })?;
    Ok(Collision {
        bounds,
        flags,
        water_z,
        meshes,
        blockers,
        animations,
        geometries,
        obstacle_models: c.array()?.0,
        prop_models: c.array()?.0,
        zone_models: c.array()?.0,
        water_volumes: c.array()?.0,
    })
}

fn vec3(c: &mut Cursor) -> Result<[f32; 3]> {
    Ok([c.f32()?, c.f32()?, c.f32()?])
}

fn list<T>(c: &mut Cursor, mut item: impl FnMut(&mut Cursor) -> Result<T>) -> Result<Vec<T>> {
    let (count, mut at) = c.array()?;
    let mut items = Vec::with_capacity(count.min(1 << 20));
    for _ in 0..count {
        items.push(item(&mut at)?);
    }
    Ok(items)
}

/// Wavefront OBJ of all meshes in model units (`(x, y, -z) * 32`).
pub fn to_obj(collision: &Collision) -> String {
    use std::fmt::Write;
    let mut out = String::from("# Guild Wars 2 collision (Havok), model units\n");
    let mut base = 1;
    for (i, mesh) in collision.meshes.iter().enumerate() {
        let _ = writeln!(out, "o collision{i}");
        for v in &mesh.vertices {
            let _ = writeln!(out, "v {} {} {}", v[0] * HAVOK_SCALE, v[1] * HAVOK_SCALE, -v[2] * HAVOK_SCALE);
        }
        for t in mesh.indices.as_chunks::<3>().0 {
            let _ = writeln!(out, "f {} {} {}", base + t[0] as usize, base + t[1] as usize, base + t[2] as usize);
        }
        base += mesh.vertices.len();
    }
    out
}
