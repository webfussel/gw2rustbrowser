//! Previews of collision (hvkC), map shadow (mpsd) and map metadata (mMet).

use gw2dat::FileType;
use gw2dat::formats::collision::{self, HAVOK_SCALE};
use gw2dat::formats::{manifests, map_data, material::token_label};
use serde::Serialize;
use tauri::ipc::Response;
use tauri::{AppHandle, Manager};

use crate::AppState;
use crate::commands::{CmdResult, blocking, err, image_response, session};
use crate::session::Session;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CollisionDto {
    meshes: Vec<CollisionMeshDto>,
    /// Blocker fences, flat xyz in model units.
    blockers: Vec<Vec<f32>>,
    animations: Vec<CollisionAnimationDto>,
    geometries: usize,
    surfaces: usize,
    obstacle_models: usize,
    prop_models: usize,
    zone_models: usize,
    water_volumes: usize,
    /// The model this collision belongs to (from the collision manifest).
    model: Option<ModelRef>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CollisionMeshDto {
    /// Flat xyz in the model's own space.
    vertices: Vec<f32>,
    indices: Vec<u16>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CollisionAnimationDto {
    name: String,
    meshes: Vec<u32>,
    blockers: Vec<u32>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ModelRef {
    id: u32,
    mft: u32,
    file_type: &'static str,
}

/// Havok metres (z up) to the model's own space (z down, 32 units per metre).
fn to_model(v: &[f32; 3]) -> [f32; 3] {
    [v[0] * HAVOK_SCALE, v[1] * HAVOK_SCALE, -v[2] * HAVOK_SCALE]
}

/// The model whose collision file this is, via the collision manifest.
fn model_of(session: &Session, mft: u32) -> Option<ModelRef> {
    let manifest = session.index.entries.iter().find(|e| e.file_type == FileType::ModelCollisionManifest)?;
    let entries = manifests::collision_manifest(&session.archive.read(manifest.mft_index).ok()?).ok()?;
    let entry = entries.iter().find(|e| e.collision.and_then(|id| session.archive.entry_for_id(id)) == Some(mft))?;
    let id = entry.model?;
    let model_mft = session.archive.entry_for_id(id)?;
    Some(ModelRef { id, mft: model_mft, file_type: session.entry(model_mft)?.file_type.name() })
}

#[tauri::command]
pub async fn collision_info(app: AppHandle, mft: u32) -> CmdResult<CollisionDto> {
    blocking(move || {
        let session = session(&app.state::<AppState>())?;
        let c = collision::decode(&session.read(mft).map_err(err)?).map_err(err)?;
        Ok(CollisionDto {
            meshes: c
                .meshes
                .iter()
                .map(|m| CollisionMeshDto {
                    vertices: m.vertices.iter().flat_map(to_model).collect(),
                    indices: m.indices.clone(),
                })
                .collect(),
            blockers: c.blockers.iter().map(|b| b.iter().flat_map(to_model).collect()).collect(),
            animations: c
                .animations
                .iter()
                .map(|a| CollisionAnimationDto {
                    name: if a.sequence == 0 { "default".into() } else { token_label(a.sequence) },
                    meshes: a.meshes.clone(),
                    blockers: a.blockers.clone(),
                })
                .collect(),
            geometries: c.geometries.len(),
            surfaces: c.meshes.iter().map(|m| m.surfaces.len()).sum(),
            obstacle_models: c.obstacle_models,
            prop_models: c.prop_models,
            zone_models: c.zone_models,
            water_volumes: c.water_volumes,
            model: model_of(&session, mft),
        })
    })
    .await
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ShadowDto {
    width: u32,
    height: u32,
    tile_width: u32,
    tile_height: u32,
    uniform_tiles: usize,
    detail_tiles: usize,
    eye: [f32; 3],
    direction: [f32; 3],
    up: [f32; 3],
}

#[tauri::command]
pub async fn shadow_info(app: AppHandle, mft: u32) -> CmdResult<ShadowDto> {
    blocking(move || {
        let session = session(&app.state::<AppState>())?;
        let s = map_data::shadow(&session.read(mft).map_err(err)?).map_err(err)?;
        let count = |f: fn(&map_data::ShadowTile) -> bool| s.tiles.iter().filter(|t| f(t)).count();
        Ok(ShadowDto {
            width: s.width,
            height: s.height,
            tile_width: s.tile_width,
            tile_height: s.tile_height,
            uniform_tiles: count(|t| matches!(t, map_data::ShadowTile::Uniform(_))),
            detail_tiles: count(|t| matches!(t, map_data::ShadowTile::Detail { .. })),
            eye: s.eye,
            direction: s.direction,
            up: s.up,
        })
    })
    .await
}

#[tauri::command]
pub async fn shadow_image(app: AppHandle, mft: u32) -> CmdResult<Response> {
    blocking(move || {
        let session = session(&app.state::<AppState>())?;
        let s = map_data::shadow(&session.read(mft).map_err(err)?).map_err(err)?;
        Ok(image_response(map_data::shadow_image(&s)))
    })
    .await
}

/// (map id, map type) pairs.
#[tauri::command]
pub async fn map_metadata(app: AppHandle, mft: u32) -> CmdResult<Vec<(u16, u8)>> {
    blocking(move || {
        let session = session(&app.state::<AppState>())?;
        map_data::metadata(&session.read(mft).map_err(err)?).map_err(err)
    })
    .await
}
