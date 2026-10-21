//! Character composition (PF `cmpc`, chunk `comp`): how player characters are
//! put together.
//!
//! The archive has one such file. Per race and sex (`AsuraMale`, …) it lists
//! the skeleton, the parts players choose in character creation (faces, hair
//! styles, beards, ears, skin styles and patterns), body and face proportions
//! (bone scales), animation overrides, preset variants with colours, and the
//! "file data": every body or armour part with its meshes, dye masks and
//! textures. The layout follows gw2formats' `PackCompositeV20`; the archive
//! stores version 19.

use super::pf::{Cursor, PackFile};
use crate::error::{Error, Result};
use crate::fourcc::fcc;

pub const CMPC: u32 = fcc(b"cmpc");

#[derive(Clone, Debug)]
pub struct Composite {
    pub version: u16,
    pub armor_colors: Vec<u32>,
    pub blit_rects: Vec<BlitRectSet>,
    pub bone_scales: Vec<BoneScale>,
    pub races: Vec<Race>,
    pub config_version: u16,
}

/// Texture atlas layout: where each part is drawn into the composed texture.
#[derive(Clone, Debug)]
pub struct BlitRectSet {
    pub name: String,
    pub size: [u32; 2],
    /// Corners: x0, y0, x1, y1.
    pub rects: Vec<[u32; 4]>,
}

#[derive(Clone, Debug)]
pub struct BoneScale {
    pub regions: Vec<BoneScaleRegion>,
    pub morph_weights: Vec<(u64, f32)>,
}

#[derive(Clone, Debug)]
pub struct BoneScaleRegion {
    pub name: u64,
    pub value: f32,
    pub bones: Vec<BoneScaleParam>,
}

#[derive(Clone, Debug)]
pub struct BoneScaleParam {
    pub name: u64,
    pub flags: u8,
    pub max: f32,
    pub min: f32,
    pub rotate: [f32; 3],
    pub scale: [f32; 3],
    pub translate: [f32; 3],
}

#[derive(Clone, Debug)]
pub struct Race {
    pub name: String,
    pub name_token: u64,
    pub base_head: u64,
    pub beards: Vec<u64>,
    pub body_bone_scales: Vec<BoneScale>,
    pub body_bone_scale_files: Vec<String>,
    pub ears: Vec<u64>,
    pub eye_color_palette: String,
    pub face_bone_scales: Vec<BoneScale>,
    pub faces: Vec<u64>,
    pub files: Vec<FileData>,
    pub flags: u32,
    pub hair_styles: Vec<u64>,
    pub hair_color_palette: String,
    pub skeleton: Option<u32>,
    pub skin_patterns: Vec<[Option<u32>; 6]>,
    pub skin_color_palette: String,
    pub skin_pattern_palette: String,
    pub skin_styles: Vec<[u64; 4]>,
    pub kind: u32,
    pub variant_of: u64,
    pub variants: Vec<Variant>,
    pub anim_overrides: Vec<(u64, Option<u32>)>,
}

/// One body or armour part.
#[derive(Clone, Debug)]
pub struct FileData {
    pub name: u64,
    pub kind: u8,
    pub flags: u8,
    pub anim_role_override: u64,
    pub mesh_base: Option<u32>,
    pub mesh_overlap: Option<u32>,
    pub dye_masks: [Option<u32>; 4],
    pub mask_cut: Option<u32>,
    pub texture_base: Option<u32>,
    pub texture_normal: Option<u32>,
    pub dye_flags: u32,
    pub hide_flags: u32,
    pub skin_flags: u32,
    pub blit_rect: u8,
}

/// HSL-style colour as stored by the game.
#[derive(Clone, Copy, Debug, Default)]
pub struct Color {
    pub brightness: u8,
    pub contrast: u8,
    pub hue: u8,
    pub saturation: u8,
    pub lightness: u8,
}

#[derive(Clone, Debug)]
pub struct Variant {
    pub token: u64,
    pub bone_scale: u32,
    pub components: Vec<(u64, [Color; 4])>,
    pub eye: Color,
    pub hair: Color,
    pub hair2: Color,
    pub pattern: Color,
    pub skin: Color,
    pub skin_index: u32,
    pub skin_style: u32,
}

pub fn decode(data: &[u8]) -> Result<Composite> {
    let file = PackFile::parse(data)?;
    if file.file_type != CMPC {
        return Err(Error::format("composite", "not a cmpc file"));
    }
    let chunk = file.require_chunk(fcc(b"comp"))?;
    let mut c = chunk.cursor();
    Ok(Composite {
        version: chunk.version,
        armor_colors: list(&mut c, |i| i.u32())?,
        blit_rects: list(&mut c, |b| {
            Ok(BlitRectSet {
                name: b.wstring()?,
                size: [b.u32()?, b.u32()?],
                rects: {
                    b.array()?; // rect index
                    list(b, |r| Ok([r.u32()?, r.u32()?, r.u32()?, r.u32()?]))?
                },
            })
        })?,
        bone_scales: list(&mut c, bone_scale)?,
        races: list(&mut c, race)?,
        config_version: c.u16()?,
    })
}

fn list<T>(c: &mut Cursor, mut item: impl FnMut(&mut Cursor) -> Result<T>) -> Result<Vec<T>> {
    let (count, mut at) = c.array()?;
    let mut items = Vec::with_capacity(count.min(1 << 16));
    for _ in 0..count {
        items.push(item(&mut at)?);
    }
    Ok(items)
}

fn vec3(c: &mut Cursor) -> Result<[f32; 3]> {
    Ok([c.f32()?, c.f32()?, c.f32()?])
}

fn bone_scale(c: &mut Cursor) -> Result<BoneScale> {
    Ok(BoneScale {
        regions: list(c, |r| {
            Ok(BoneScaleRegion {
                name: r.u64()?,
                value: r.f32()?,
                bones: list(r, |b| {
                    Ok(BoneScaleParam {
                        name: b.u64()?,
                        flags: b.u8()?,
                        max: b.f32()?,
                        min: b.f32()?,
                        rotate: vec3(b)?,
                        scale: vec3(b)?,
                        translate: vec3(b)?,
                    })
                })?,
            })
        })?,
        morph_weights: list(c, |m| Ok((m.u64()?, m.f32()?)))?,
    })
}

fn color(c: &mut Cursor) -> Result<Color> {
    Ok(Color { brightness: c.u8()?, contrast: c.u8()?, hue: c.u8()?, saturation: c.u8()?, lightness: c.u8()? })
}

fn race(c: &mut Cursor) -> Result<Race> {
    Ok(Race {
        name: c.wstring()?,
        name_token: c.u64()?,
        base_head: c.u64()?,
        beards: list(c, |i| i.u64())?,
        body_bone_scales: list(c, bone_scale)?,
        body_bone_scale_files: list(c, |f| f.wstring())?,
        ears: list(c, |i| i.u64())?,
        eye_color_palette: c.wstring()?,
        face_bone_scales: list(c, bone_scale)?,
        faces: list(c, |i| i.u64())?,
        files: list(c, |f| {
            Ok(FileData {
                name: f.u64()?,
                kind: f.u8()?,
                flags: f.u8()?,
                anim_role_override: f.u64()?,
                mesh_base: f.file_ref()?,
                mesh_overlap: f.file_ref()?,
                dye_masks: [f.file_ref()?, f.file_ref()?, f.file_ref()?, f.file_ref()?],
                mask_cut: f.file_ref()?,
                texture_base: f.file_ref()?,
                texture_normal: f.file_ref()?,
                dye_flags: f.u32()?,
                hide_flags: f.u32()?,
                skin_flags: f.u32()?,
                blit_rect: f.u8()?,
            })
        })?,
        flags: c.u32()?,
        hair_styles: list(c, |i| i.u64())?,
        hair_color_palette: c.wstring()?,
        skeleton: c.file_ref()?,
        skin_patterns: list(c, |p| {
            Ok([p.file_ref()?, p.file_ref()?, p.file_ref()?, p.file_ref()?, p.file_ref()?, p.file_ref()?])
        })?,
        skin_color_palette: c.wstring()?,
        skin_pattern_palette: c.wstring()?,
        skin_styles: list(c, |s| Ok([s.u64()?, s.u64()?, s.u64()?, s.u64()?]))?,
        kind: c.u32()?,
        variant_of: c.u64()?,
        variants: list(c, |v| {
            Ok(Variant {
                token: v.u64()?,
                bone_scale: v.u32()?,
                components: list(v, |m| Ok((m.u64()?, [color(m)?, color(m)?, color(m)?, color(m)?])))?,
                eye: color(v)?,
                hair: color(v)?,
                hair2: color(v)?,
                pattern: color(v)?,
                skin: color(v)?,
                skin_index: v.u32()?,
                skin_style: v.u32()?,
            })
        })?,
        anim_overrides: list(c, |a| Ok((a.u64()?, a.file_ref()?)))?,
    })
}
