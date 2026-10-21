//! Preview of the character composition file (cmpc).

use gw2dat::formats::composite::{self, Color};
use gw2dat::formats::material::token_label;
use serde::Serialize;
use tauri::{AppHandle, Manager};

use crate::AppState;
use crate::commands::{CmdResult, blocking, err, session};
use crate::session::Session;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileRefDto {
    id: u32,
    mft: i64,
    kind: Option<&'static str>,
}

fn file(session: &Session, id: Option<u32>) -> Option<FileRefDto> {
    let id = id?;
    let mft = session.archive.entry_for_id(id);
    Some(FileRefDto {
        id,
        mft: mft.map_or(-1, i64::from),
        kind: mft.and_then(|m| session.entry(m)).map(|e| e.file_type.name()),
    })
}

fn name(token: u64) -> String {
    if token == 0 { String::new() } else { token_label(token) }
}

fn names(tokens: &[u64]) -> Vec<String> {
    tokens.iter().map(|&t| name(t)).collect()
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompositeDto {
    version: u16,
    config_version: u16,
    bone_scales: usize,
    blit_rects: Vec<BlitRectDto>,
    races: Vec<RaceSummaryDto>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct BlitRectDto {
    name: String,
    size: [u32; 2],
    /// Corners: x0, y0, x1, y1.
    rects: Vec<[u32; 4]>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct RaceSummaryDto {
    name: String,
    parts: usize,
    variants: usize,
    faces: usize,
    hair_styles: usize,
}

#[tauri::command]
pub async fn composite_info(app: AppHandle, mft: u32) -> CmdResult<CompositeDto> {
    blocking(move || {
        let session = session(&app.state::<AppState>())?;
        info(&session.read(mft).map_err(err)?)
    })
    .await
}

fn info(data: &[u8]) -> CmdResult<CompositeDto> {
    let c = composite::decode(data).map_err(err)?;
    {
        Ok(CompositeDto {
            version: c.version,
            config_version: c.config_version,
            bone_scales: c.bone_scales.len(),
            blit_rects: c
                .blit_rects
                .iter()
                .map(|b| BlitRectDto { name: b.name.clone(), size: b.size, rects: b.rects.clone() })
                .collect(),
            races: c
                .races
                .iter()
                .map(|r| RaceSummaryDto {
                    name: r.name.clone(),
                    parts: r.files.len(),
                    variants: r.variants.len(),
                    faces: r.faces.len(),
                    hair_styles: r.hair_styles.len(),
                })
                .collect(),
        })
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RaceDto {
    name: String,
    skeleton: Option<FileRefDto>,
    flags: u32,
    kind: u32,
    variant_of: String,
    palettes: [String; 3],
    skin_pattern_palette: String,
    faces: Vec<String>,
    hair_styles: Vec<String>,
    beards: Vec<String>,
    ears: Vec<String>,
    skin_styles: Vec<Vec<String>>,
    /// chest, face, feet, hands, legs, ears.
    skin_patterns: Vec<Vec<Option<FileRefDto>>>,
    body_shapes: Vec<String>,
    face_bone_scales: usize,
    anim_overrides: Vec<(String, Option<FileRefDto>)>,
    parts: Vec<PartDto>,
    variants: Vec<VariantDto>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PartDto {
    name: String,
    kind: u8,
    flags: u8,
    mesh: Option<FileRefDto>,
    mesh_overlap: Option<FileRefDto>,
    dye_masks: Vec<Option<FileRefDto>>,
    cut_mask: Option<FileRefDto>,
    texture: Option<FileRefDto>,
    normal: Option<FileRefDto>,
    blit_rect: u8,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct VariantDto {
    name: String,
    bone_scale: u32,
    /// eye, hair, hair 2, pattern, skin.
    colors: [[u8; 5]; 5],
    components: Vec<(String, [[u8; 5]; 4])>,
    skin_index: u32,
    skin_style: u32,
}

fn color(c: &Color) -> [u8; 5] {
    [c.brightness, c.contrast, c.hue, c.saturation, c.lightness]
}

#[tauri::command]
pub async fn composite_race(app: AppHandle, mft: u32, index: usize) -> CmdResult<RaceDto> {
    blocking(move || {
        let session = session(&app.state::<AppState>())?;
        race(&session, &session.read(mft).map_err(err)?, index)
    })
    .await
}

fn race(s: &Session, data: &[u8], index: usize) -> CmdResult<RaceDto> {
    let c = composite::decode(data).map_err(err)?;
    let r = c.races.get(index).ok_or("No such race")?;
    {
        Ok(RaceDto {
            name: r.name.clone(),
            skeleton: file(s, r.skeleton),
            flags: r.flags,
            kind: r.kind,
            variant_of: name(r.variant_of),
            palettes: [r.eye_color_palette.clone(), r.hair_color_palette.clone(), r.skin_color_palette.clone()],
            skin_pattern_palette: r.skin_pattern_palette.clone(),
            faces: names(&r.faces),
            hair_styles: names(&r.hair_styles),
            beards: names(&r.beards),
            ears: names(&r.ears),
            skin_styles: r.skin_styles.iter().map(|st| names(st)).collect(),
            skin_patterns: r.skin_patterns.iter().map(|p| p.iter().map(|&id| file(s, id)).collect()).collect(),
            body_shapes: r.body_bone_scale_files.clone(),
            face_bone_scales: r.face_bone_scales.len(),
            anim_overrides: r.anim_overrides.iter().map(|&(role, f)| (name(role), file(s, f))).collect(),
            parts: r
                .files
                .iter()
                .map(|f| PartDto {
                    name: name(f.name),
                    kind: f.kind,
                    flags: f.flags,
                    mesh: file(s, f.mesh_base),
                    mesh_overlap: file(s, f.mesh_overlap),
                    dye_masks: f.dye_masks.iter().map(|&id| file(s, id)).collect(),
                    cut_mask: file(s, f.mask_cut),
                    texture: file(s, f.texture_base),
                    normal: file(s, f.texture_normal),
                    blit_rect: f.blit_rect,
                })
                .collect(),
            variants: r
                .variants
                .iter()
                .map(|v| VariantDto {
                    name: name(v.token),
                    bone_scale: v.bone_scale,
                    colors: [color(&v.eye), color(&v.hair), color(&v.hair2), color(&v.pattern), color(&v.skin)],
                    components: v.components.iter().map(|(t, cs)| (name(*t), cs.map(|c| color(&c)))).collect(),
                    skin_index: v.skin_index,
                    skin_style: v.skin_style,
                })
                .collect(),
        })
    }
}
