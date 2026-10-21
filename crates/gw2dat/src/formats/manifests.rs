//! Content manifests: model collision (PF `cmaC`) and content portals (`prlt`).
//!
//! - The collision manifest (one file) lists every model with collision: its
//!   original asset path from development, the model file, the Havok collision
//!   file (`hvkC`) and optional scales.
//! - Portal manifests list, per map (by GUID), where players start and which
//!   models the game loads there, plus redirections to other maps. The GUIDs
//!   are not those in the map files' `parm` chunk.

use super::pf::{Cursor, PackFile};
use crate::error::{Error, Result};
use crate::fourcc::fcc;

pub const CMAC: u32 = fcc(b"cmaC");
pub const PRLT: u32 = fcc(b"prlt");

#[derive(Clone, Debug)]
pub struct CollisionEntry {
    /// Asset path, e.g. `Cinema\Cinematics\…\name.amdl2`.
    pub name: String,
    pub model: Option<u32>,
    pub collision: Option<u32>,
    pub scales: Vec<f32>,
}

pub fn collision_manifest(data: &[u8]) -> Result<Vec<CollisionEntry>> {
    let file = PackFile::parse(data)?;
    if file.file_type != CMAC {
        return Err(Error::format("collision manifest", "not a cmaC file"));
    }
    let mut c = file.require_chunk(fcc(b"main"))?.cursor();
    let (count, mut item) = c.array()?;
    let mut entries = Vec::with_capacity(count.min(1 << 20));
    for _ in 0..count {
        let name = item.wstring()?;
        let model = item.file_ref()?;
        let collision = item.file_ref()?;
        let (scales, mut s) = item.array()?;
        let scales = (0..scales).map(|_| s.f32()).collect::<Result<_>>()?;
        entries.push(CollisionEntry { name, model, collision, scales });
    }
    Ok(entries)
}

#[derive(Clone, Debug)]
pub struct PortalMap {
    pub guid: [u8; 16],
    pub redirectors: Vec<Redirector>,
    pub starts: Vec<MapStart>,
}

/// A spot that sends players to another map.
#[derive(Clone, Debug)]
pub struct Redirector {
    pub guid: [u8; 16],
    pub token: u32,
    pub position: [f32; 3],
    pub radius: f32,
}

/// A starting point and the models loaded for it.
#[derive(Clone, Debug)]
pub struct MapStart {
    pub token: u32,
    pub models: Vec<StartModel>,
    pub position: [f32; 3],
    pub radius: f32,
}

#[derive(Clone, Debug)]
pub struct StartModel {
    pub file: Option<u32>,
    pub flags: u32,
    pub kind: u32,
    pub permutation: u64,
}

pub fn portal_manifest(data: &[u8]) -> Result<Vec<PortalMap>> {
    let file = PackFile::parse(data)?;
    if file.file_type != PRLT {
        return Err(Error::format("portal manifest", "not a prlt file"));
    }
    let mut c = file.require_chunk(fcc(b"mfst"))?.cursor();
    let (count, mut item) = c.array()?;
    let mut maps = Vec::with_capacity(count.min(1 << 16));
    for _ in 0..count {
        let guid = read_guid(&mut item)?;
        let (n, mut r) = item.array()?;
        let mut redirectors = Vec::with_capacity(n.min(1 << 16));
        for _ in 0..n {
            redirectors.push(Redirector {
                guid: read_guid(&mut r)?,
                token: r.u32()?,
                position: vec3(&mut r)?,
                radius: r.f32()?,
            });
        }
        let (n, mut s) = item.array()?;
        let mut starts = Vec::with_capacity(n.min(1 << 16));
        for _ in 0..n {
            let token = s.u32()?;
            let (models, mut m) = s.array()?;
            let models = (0..models)
                .map(|_| Ok(StartModel { file: m.file_ref()?, flags: m.u32()?, kind: m.u32()?, permutation: m.u64()? }))
                .collect::<Result<_>>()?;
            starts.push(MapStart { token, models, position: vec3(&mut s)?, radius: s.f32()? });
        }
        maps.push(PortalMap { guid, redirectors, starts });
    }
    Ok(maps)
}

fn read_guid(c: &mut Cursor) -> Result<[u8; 16]> {
    Ok(c.take(16)?.try_into().unwrap())
}

fn vec3(c: &mut Cursor) -> Result<[f32; 3]> {
    Ok([c.f32()?, c.f32()?, c.f32()?])
}

/// Windows notation, e.g. `{d76129c0-db3b-4a23-883b-f54b6c71015b}`.
pub fn guid_text(guid: &[u8; 16]) -> String {
    let d1 = u32::from_le_bytes(guid[0..4].try_into().unwrap());
    let d2 = u16::from_le_bytes([guid[4], guid[5]]);
    let d3 = u16::from_le_bytes([guid[6], guid[7]]);
    let tail: String = guid[8..].iter().map(|b| format!("{b:02x}")).collect();
    format!("{{{d1:08x}-{d2:04x}-{d3:04x}-{}-{}}}", &tail[..4], &tail[4..])
}

/// `name,model,collision,scales` with a header.
pub fn collision_csv(entries: &[CollisionEntry]) -> String {
    let mut out = String::from("name,model,collision,scales\n");
    let id = |f: Option<u32>| f.map_or(String::new(), |v| v.to_string());
    for e in entries {
        let scales: Vec<String> = e.scales.iter().map(|s| s.to_string()).collect();
        out.push_str(&format!(
            "\"{}\",{},{},{}\n",
            e.name.replace('"', "\"\""),
            id(e.model),
            id(e.collision),
            scales.join(" ")
        ));
    }
    out
}

/// One row per start model: `map,start,token,x,y,z,radius,model,flags`.
pub fn portal_csv(maps: &[PortalMap]) -> String {
    let mut out = String::from("map,start,token,x,y,z,radius,model,flags\n");
    for map in maps {
        let guid = guid_text(&map.guid);
        for (i, s) in map.starts.iter().enumerate() {
            let [x, y, z] = s.position;
            for m in &s.models {
                let file = m.file.map_or(String::new(), |v| v.to_string());
                out.push_str(&format!("{guid},{i},{:#x},{x},{y},{z},{},{file},{}\n", s.token, s.radius, m.flags));
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn guid_notation() {
        let guid = [0xc0, 0x29, 0x61, 0xd7, 0x3b, 0xdb, 0x23, 0x4a, 0x88, 0x3b, 0xf5, 0x4b, 0x6c, 0x71, 0x01, 0x5b];
        assert_eq!(guid_text(&guid), "{d76129c0-db3b-4a23-883b-f54b6c71015b}");
    }
}
