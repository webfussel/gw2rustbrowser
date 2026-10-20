//! Model geometry from PF `MODL` files (`GEOM` chunk, version 1).

use super::pf::{Cursor, PackFile};
use crate::error::{Error, Result};
use crate::fourcc::fcc;

pub const MODL: u32 = fcc(b"MODL");
pub const GEOM: u32 = fcc(b"GEOM");

/// Flexible vertex format bits.
mod fvf {
    pub const POSITION: u32 = 0x0000_0001;
    pub const WEIGHTS: u32 = 0x0000_0002;
    pub const GROUP: u32 = 0x0000_0004;
    pub const NORMAL: u32 = 0x0000_0008;
    pub const COLOR: u32 = 0x0000_0010;
    pub const TANGENT: u32 = 0x0000_0020;
    pub const BITANGENT: u32 = 0x0000_0040;
    pub const TANGENT_FRAME: u32 = 0x0000_0080;
    pub const UV32_MASK: u32 = 0x0000_ff00;
    pub const UV16_MASK: u32 = 0x00ff_0000;
    pub const UNKNOWN_48: u32 = 0x0100_0000;
    pub const UNKNOWN_4A: u32 = 0x0200_0000;
    pub const UNKNOWN_4B: u32 = 0x0400_0000;
    pub const UNKNOWN_16: u32 = 0x0800_0000;
    pub const POSITION_COMPRESSED: u32 = 0x1000_0000;
    pub const UNKNOWN_12: u32 = 0x2000_0000;
}

#[derive(Clone, Debug, Default)]
pub struct Mesh {
    pub material_index: u32,
    pub material_name: String,
    /// Positions in a right-handed, Y-up coordinate system.
    pub positions: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    /// First texture coordinate set, if present.
    pub uvs: Vec<[f32; 2]>,
    /// Triangle list.
    pub indices: Vec<u32>,
}

#[derive(Clone, Debug, Default)]
pub struct Model {
    pub meshes: Vec<Mesh>,
}

impl Model {
    pub fn vertex_count(&self) -> usize {
        self.meshes.iter().map(|m| m.positions.len()).sum()
    }

    pub fn triangle_count(&self) -> usize {
        self.meshes.iter().map(|m| m.indices.len() / 3).sum()
    }

    pub fn bounds(&self) -> Option<([f32; 3], [f32; 3])> {
        let mut points = self.meshes.iter().flat_map(|m| m.positions.iter());
        let first = *points.next()?;
        Some(points.fold((first, first), |(mut lo, mut hi), p| {
            for i in 0..3 {
                lo[i] = lo[i].min(p[i]);
                hi[i] = hi[i].max(p[i]);
            }
            (lo, hi)
        }))
    }
}

pub fn vertex_size(format: u32) -> usize {
    let flag = |bit: u32, size: usize| if format & bit != 0 { size } else { 0 };
    flag(fvf::POSITION, 12)
        + flag(fvf::WEIGHTS, 4)
        + flag(fvf::GROUP, 4)
        + flag(fvf::NORMAL, 12)
        + flag(fvf::COLOR, 4)
        + flag(fvf::TANGENT, 12)
        + flag(fvf::BITANGENT, 12)
        + flag(fvf::TANGENT_FRAME, 12)
        + (format & fvf::UV32_MASK).count_ones() as usize * 8
        + (format & fvf::UV16_MASK).count_ones() as usize * 4
        + flag(fvf::UNKNOWN_48, 48)
        + flag(fvf::UNKNOWN_4A, 4)
        + flag(fvf::UNKNOWN_4B, 4)
        + flag(fvf::UNKNOWN_16, 16)
        + flag(fvf::POSITION_COMPRESSED, 6)
        + flag(fvf::UNKNOWN_12, 12)
}

pub fn f16_to_f32(bits: u16) -> f32 {
    let sign = if bits & 0x8000 != 0 { -1.0 } else { 1.0 };
    let exponent = ((bits >> 10) & 0x1f) as i32;
    let mantissa = (bits & 0x3ff) as f32;
    match exponent {
        0 => sign * mantissa * 2f32.powi(-24),
        31 => {
            if mantissa == 0.0 {
                sign * f32::INFINITY
            } else {
                f32::NAN
            }
        }
        e => sign * (1.0 + mantissa / 1024.0) * 2f32.powi(e - 15),
    }
}

fn f32_at(b: &[u8], at: usize) -> f32 {
    f32::from_le_bytes(b[at..at + 4].try_into().unwrap())
}

fn vec3_at(b: &[u8], at: usize) -> [f32; 3] {
    [f32_at(b, at), f32_at(b, at + 4), f32_at(b, at + 8)]
}

/// DirectX (Z-down) to OpenGL (Y-up): `(x, y, z) -> (x, -z, -y)`. This mirrors
/// the model, so triangle winding is flipped as well.
fn to_gl(v: [f32; 3]) -> [f32; 3] {
    [v[0], -v[2], -v[1]]
}

fn decode_vertices(format: u32, count: usize, data: &[u8], mesh: &mut Mesh) -> Result<()> {
    let stride = vertex_size(format);
    if stride == 0 || data.len() < stride * count {
        return Err(Error::format("model", format!("vertex buffer too small (fvf {format:#x})")));
    }
    let has_normal = format & fvf::NORMAL != 0;
    let has_uv = format & (fvf::UV32_MASK | fvf::UV16_MASK) != 0;
    mesh.positions.reserve(count);
    if has_normal {
        mesh.normals.reserve(count);
    }
    if has_uv {
        mesh.uvs.reserve(count);
    }

    for vertex in data.chunks_exact(stride).take(count) {
        let mut pos = 0;
        let mut position = [0.0; 3];
        let mut uv: Option<[f32; 2]> = None;
        if format & fvf::POSITION != 0 {
            position = vec3_at(vertex, pos);
            pos += 12;
        }
        if format & fvf::WEIGHTS != 0 {
            pos += 4;
        }
        if format & fvf::GROUP != 0 {
            pos += 4;
        }
        if has_normal {
            let n = to_gl(vec3_at(vertex, pos));
            let length = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
            mesh.normals.push(if length > 0.0 {
                [n[0] / length, n[1] / length, n[2] / length]
            } else {
                [0.0, 1.0, 0.0]
            });
            pos += 12;
        }
        if format & fvf::COLOR != 0 {
            pos += 4;
        }
        for bit in [fvf::TANGENT, fvf::BITANGENT, fvf::TANGENT_FRAME] {
            if format & bit != 0 {
                pos += 12;
            }
        }
        for _ in 0..(format & fvf::UV32_MASK).count_ones() {
            uv.get_or_insert([f32_at(vertex, pos), f32_at(vertex, pos + 4)]);
            pos += 8;
        }
        for _ in 0..(format & fvf::UV16_MASK).count_ones() {
            let u = f16_to_f32(u16::from_le_bytes([vertex[pos], vertex[pos + 1]]));
            let v = f16_to_f32(u16::from_le_bytes([vertex[pos + 2], vertex[pos + 3]]));
            uv.get_or_insert([u, v]);
            pos += 4;
        }
        for (bit, size) in [(fvf::UNKNOWN_48, 48), (fvf::UNKNOWN_4A, 4), (fvf::UNKNOWN_4B, 4), (fvf::UNKNOWN_16, 16)] {
            if format & bit != 0 {
                pos += size;
            }
        }
        if format & fvf::POSITION_COMPRESSED != 0 {
            let half = |at: usize| f16_to_f32(u16::from_le_bytes([vertex[at], vertex[at + 1]]));
            position = [half(pos), half(pos + 2), half(pos + 4)];
        }
        mesh.positions.push(to_gl(position));
        if has_uv {
            mesh.uvs.push(uv.unwrap_or_default());
        }
    }
    Ok(())
}

fn read_mesh(mut c: Cursor) -> Result<Option<Mesh>> {
    let ptr = c.pointer_size();
    c.skip(8)?; // visibility bone
    c.skip(4 + ptr)?; // morph targets
    c.skip(4)?; // flags
    c.skip(4 + ptr)?; // seam vertex indices
    c.skip(8)?; // mesh name token
    c.skip(24)?; // min/max bound
    c.skip(4 + ptr)?; // bounds
    let material_index = c.u32()?;
    let material_name = c.string()?;
    c.skip(4 + ptr)?; // bone bindings
    let Some(mut geometry) = c.ptr()? else { return Ok(None) };

    let vertex_count = geometry.u32()? as usize;
    let format = geometry.u32()?;
    let vertices = geometry.byte_array()?;
    let (index_count, mut index_data) = geometry.array()?;

    let mut mesh = Mesh { material_index, material_name, ..Mesh::default() };
    if vertex_count > 0 {
        decode_vertices(format, vertex_count, vertices, &mut mesh)?;
    }
    let raw = index_data.take(index_count * 2)?;
    mesh.indices.reserve(index_count);
    for triangle in raw.as_chunks::<6>().0 {
        let index = |i: usize| u16::from_le_bytes([triangle[i], triangle[i + 1]]) as u32;
        let (a, b, c) = (index(0), index(2), index(4));
        if a as usize >= vertex_count || b as usize >= vertex_count || c as usize >= vertex_count {
            return Err(Error::format("model", "triangle index out of range"));
        }
        // Flipped winding, see `to_gl`.
        mesh.indices.extend_from_slice(&[a, c, b]);
    }
    Ok(Some(mesh))
}

/// Reads all meshes (highest level of detail) of a model file.
pub fn decode(data: &[u8]) -> Result<Model> {
    let file = PackFile::parse(data)?;
    if file.file_type != MODL {
        return Err(Error::format("model", "not a MODL file"));
    }
    let chunk = file.require_chunk(GEOM)?;
    if chunk.version != 1 {
        return Err(Error::Unsupported(format!("GEOM chunk version {}", chunk.version)));
    }
    let mut meshes = Vec::new();
    for item in chunk.cursor().ref_list()? {
        if let Some(mesh) = read_mesh(item)? {
            meshes.push(mesh);
        }
    }
    Ok(Model { meshes })
}

/// Writes the model as Wavefront OBJ (one group per mesh).
pub fn to_obj(model: &Model, name: &str) -> String {
    use std::fmt::Write;
    let mut out = String::new();
    if name.is_empty() {
        let _ = writeln!(out, "# Guild Wars 2 model, exported by GW2 Browser");
    } else {
        let _ = writeln!(out, "# Guild Wars 2 model {name}, exported by GW2 Browser");
    }
    let mut base = 1usize;
    for (i, mesh) in model.meshes.iter().enumerate() {
        let _ = writeln!(out, "o mesh_{i}");
        if !mesh.material_name.is_empty() {
            let _ = writeln!(out, "# material {} ({})", mesh.material_index, mesh.material_name);
        }
        for p in &mesh.positions {
            let _ = writeln!(out, "v {} {} {}", p[0], p[1], p[2]);
        }
        for t in &mesh.uvs {
            let _ = writeln!(out, "vt {} {}", t[0], 1.0 - t[1]);
        }
        for n in &mesh.normals {
            let _ = writeln!(out, "vn {} {} {}", n[0], n[1], n[2]);
        }
        let (has_uv, has_normal) = (!mesh.uvs.is_empty(), !mesh.normals.is_empty());
        for tri in mesh.indices.as_chunks::<3>().0 {
            out.push('f');
            for &index in tri {
                let v = base + index as usize;
                match (has_uv, has_normal) {
                    (true, true) => write!(out, " {v}/{v}/{v}"),
                    (true, false) => write!(out, " {v}/{v}"),
                    (false, true) => write!(out, " {v}//{v}"),
                    (false, false) => write!(out, " {v}"),
                }
                .ok();
            }
            out.push('\n');
        }
        base += mesh.positions.len();
    }
    out
}

/// Compact binary form for the viewer. All sections are 4-byte aligned:
/// `[u32 mesh count]`, then per mesh `[u32 vertices][u32 indices][u32 flags:
/// 1 = normals, 2 = uvs][u32 material index]`, positions (f32×3), normals
/// (f32×3, if flagged), uvs (f32×2, if flagged), indices (u32).
pub fn to_binary(model: &Model) -> Vec<u8> {
    let mut out = Vec::new();
    let push_u32 = |out: &mut Vec<u8>, v: u32| out.extend_from_slice(&v.to_le_bytes());
    push_u32(&mut out, model.meshes.len() as u32);
    for mesh in &model.meshes {
        let has_normals = mesh.normals.len() == mesh.positions.len() && !mesh.normals.is_empty();
        let has_uvs = mesh.uvs.len() == mesh.positions.len() && !mesh.uvs.is_empty();
        push_u32(&mut out, mesh.positions.len() as u32);
        push_u32(&mut out, mesh.indices.len() as u32);
        push_u32(&mut out, has_normals as u32 | (has_uvs as u32) << 1);
        push_u32(&mut out, mesh.material_index);
        for v in mesh.positions.iter().flatten() {
            out.extend_from_slice(&v.to_le_bytes());
        }
        if has_normals {
            for v in mesh.normals.iter().flatten() {
                out.extend_from_slice(&v.to_le_bytes());
            }
        }
        if has_uvs {
            for v in mesh.uvs.iter().flatten() {
                out.extend_from_slice(&v.to_le_bytes());
            }
        }
        for &i in &mesh.indices {
            push_u32(&mut out, i);
        }
    }
    out
}

pub const MODL_CHUNK: u32 = fcc(b"MODL");

/// Texture slots identified by the low 32 bits of the material token.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextureSlot {
    Diffuse,
    Normal,
    Specular,
    LightMap,
    Dye,
    Other,
}

impl TextureSlot {
    fn from_token(token: u64) -> TextureSlot {
        match token as u32 {
            0x6753_1924 => TextureSlot::Diffuse,
            0x1816_c9ee => TextureSlot::Normal,
            0x5951_9613 => TextureSlot::Specular,
            0x680b_bd87 => TextureSlot::LightMap,
            0xe616_9724 => TextureSlot::Dye,
            _ => TextureSlot::Other,
        }
    }
}

#[derive(Clone, Debug)]
pub struct TextureRef {
    pub slot: TextureSlot,
    pub token: u64,
    pub file_id: u32,
}

#[derive(Clone, Debug, Default)]
pub struct Material {
    pub material_id: u32,
    /// The compiled material (AMAT) file.
    pub material_file: Option<u32>,
    pub flags: u32,
    pub textures: Vec<TextureRef>,
}

impl Material {
    pub fn texture(&self, slot: TextureSlot) -> Option<u32> {
        self.textures.iter().find(|t| t.slot == slot).map(|t| t.file_id)
    }
}

fn read_material(mut c: Cursor) -> Result<Material> {
    c.skip(8)?; // token
    let material_id = c.u32()?;
    let material_file = c.file_ref()?;
    let flags = c.u32()?;
    c.skip(4)?; // sort order
    let (count, mut texture) = c.array()?;
    let mut textures = Vec::with_capacity(count.min(64));
    for _ in 0..count {
        let file_id = texture.file_ref()?;
        texture.skip(4)?; // texture flags
        let token = texture.u64()?;
        texture.skip(8 + 4 + 1)?; // blit id, uv animation id, uv input index
        // Texture references point one id below the texture file.
        if let Some(file_id) = file_id {
            textures.push(TextureRef { slot: TextureSlot::from_token(token), token, file_id: file_id + 1 });
        }
    }
    Ok(Material { material_id, material_file, flags, textures })
}

/// Reads the materials of a model, indexed like `Mesh::material_index`. Each
/// index takes the first definition found in any permutation.
pub fn materials(data: &[u8]) -> Result<Vec<Material>> {
    let file = PackFile::parse(data)?;
    let chunk = file.require_chunk(MODL_CHUNK)?;
    let (permutations, mut permutation) = chunk.cursor().array()?;
    let mut materials: Vec<Option<Material>> = Vec::new();
    for _ in 0..permutations.min(4096) {
        permutation.skip(8)?; // token
        let entries = permutation.ref_list()?;
        if materials.len() < entries.len() {
            materials.resize(entries.len(), None);
        }
        for (slot, entry) in materials.iter_mut().zip(entries) {
            if slot.is_none() {
                *slot = Some(read_material(entry)?);
            }
        }
    }
    Ok(materials.into_iter().map(Option::unwrap_or_default).collect())
}

/// OBJ referencing an MTL file with one material per used material index.
/// `texture_file` names the exported image for a texture file id.
pub fn to_obj_mtl(
    model: &Model,
    materials: &[Material],
    name: &str,
    texture_file: impl Fn(u32) -> String,
) -> (String, String) {
    use std::fmt::Write;
    let obj = to_obj(model, name);
    // Insert the material library and `usemtl` lines into the plain OBJ.
    let mut out = String::with_capacity(obj.len() + 256);
    let _ = writeln!(out, "mtllib {name}.mtl");
    let mut mesh = 0usize;
    for line in obj.lines() {
        out.push_str(line);
        out.push('\n');
        if line.starts_with("o mesh_") {
            let _ = writeln!(out, "usemtl material_{}", model.meshes[mesh].material_index);
            mesh += 1;
        }
    }

    let mut mtl = String::from("# Materials exported by GW2 Browser\n");
    let mut used: Vec<u32> = model.meshes.iter().map(|m| m.material_index).collect();
    used.sort_unstable();
    used.dedup();
    for index in used {
        let _ = writeln!(mtl, "\nnewmtl material_{index}\nKd 1 1 1\nKs 0 0 0\nillum 1");
        if let Some(material) = materials.get(index as usize) {
            if let Some(id) = material.texture(TextureSlot::Diffuse) {
                let _ = writeln!(mtl, "map_Kd {}", texture_file(id));
            }
            if let Some(id) = material.texture(TextureSlot::Normal) {
                let _ = writeln!(mtl, "map_Bump {}", texture_file(id));
            }
        }
    }
    (out, mtl)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn half_floats() {
        assert_eq!(f16_to_f32(0x3c00), 1.0);
        assert_eq!(f16_to_f32(0xc000), -2.0);
        assert_eq!(f16_to_f32(0x0000), 0.0);
        assert!((f16_to_f32(0x3555) - 0.333).abs() < 1e-3);
    }

    #[test]
    fn vertex_sizes() {
        assert_eq!(vertex_size(fvf::POSITION), 12);
        assert_eq!(vertex_size(fvf::POSITION | fvf::NORMAL | 0x0000_0100), 32);
        assert_eq!(vertex_size(fvf::POSITION_COMPRESSED | 0x0003_0000), 14);
    }

    #[test]
    fn obj_export_is_one_based() {
        let model = Model {
            meshes: vec![Mesh {
                positions: vec![[0.0; 3], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
                indices: vec![0, 1, 2],
                ..Mesh::default()
            }],
        };
        let obj = to_obj(&model, "test");
        assert!(obj.contains("\nf 1 2 3\n"));
    }
}
