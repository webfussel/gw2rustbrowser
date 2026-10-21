//! Map preview: terrain, props and water in one binary message, prop models
//! on demand.

use std::collections::HashMap;

use gw2dat::Kind;
use gw2dat::formats::{self, map::PropKind};
use serde::Serialize;
use tauri::ipc::Response;
use tauri::{AppHandle, Manager};

use crate::AppState;
use crate::commands::{CmdResult, blocking, err, session};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct MapHeader {
    rect: [f32; 4],
    columns: usize,
    rows: usize,
    z_range: Option<(f32, f32)>,
    /// File id per model index used by the props.
    models: Vec<u32>,
    prop_count: usize,
    /// Props per kind: static, animated, instance copies, group members.
    prop_kinds: [usize; 4],
    /// Props whose model reference is missing or not a model file.
    missing_models: usize,
    water: Option<WaterDto>,
    chunks: Vec<ChunkDto>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct WaterDto {
    plane: bool,
    plane_z: f32,
    surfaces: Vec<WaterSurfaceDto>,
}

#[derive(Serialize)]
struct WaterSurfaceDto {
    z: f32,
    outline: Vec<[f32; 2]>,
}

#[derive(Serialize)]
struct ChunkDto {
    name: String,
    version: u16,
    size: usize,
}

/// `[header length u32][header JSON][padding to 4][heights f32 × columns × rows]
/// [props: model index u32, position 3×f32, rotation 3×f32, scale f32]`.
#[tauri::command]
pub async fn load_map(app: AppHandle, mft: u32) -> CmdResult<Response> {
    blocking(move || {
        let session = session(&app.state::<AppState>())?;
        let data = session.read(mft).map_err(err)?;
        let map = formats::map::decode(&data).map_err(err)?;

        let mut model_index: HashMap<u32, u32> = HashMap::new();
        let mut models = Vec::new();
        let mut placed = Vec::with_capacity(map.props.len());
        let mut missing = 0;
        let mut kinds = [0; 4];
        for prop in &map.props {
            let is_model = prop.model.and_then(|id| session.archive.entry_for_id(id)).and_then(|m| session.entry(m));
            let Some(id) = prop.model.filter(|_| is_model.is_some_and(|e| e.file_type.kind() == Kind::Model)) else {
                missing += 1;
                continue;
            };
            let index = *model_index.entry(id).or_insert_with(|| {
                models.push(id);
                models.len() as u32 - 1
            });
            kinds[match prop.kind {
                PropKind::Static => 0,
                PropKind::Animated => 1,
                PropKind::Instance => 2,
                PropKind::Meta => 3,
            }] += 1;
            placed.push((index, prop));
        }

        let terrain = map.terrain.as_ref();
        let header = MapHeader {
            rect: map.rect,
            columns: terrain.map_or(0, |t| t.columns),
            rows: terrain.map_or(0, |t| t.rows),
            z_range: terrain.and_then(|t| t.z_range()),
            models,
            prop_count: placed.len(),
            prop_kinds: kinds,
            missing_models: missing,
            water: map.water.as_ref().map(|w| WaterDto {
                plane: w.flags & 1 != 0,
                plane_z: w.plane_z,
                surfaces: w
                    .surfaces
                    .iter()
                    .filter(|s| s.outline.len() >= 3)
                    .map(|s| WaterSurfaceDto { z: s.z, outline: s.outline.clone() })
                    .collect(),
            }),
            chunks: map
                .chunks
                .iter()
                .map(|(name, version, size)| ChunkDto { name: name.clone(), version: *version, size: *size })
                .collect(),
        };
        let json = serde_json::to_vec(&header).map_err(err)?;
        let heights = terrain.map_or(&[][..], |t| &t.heights);
        let mut out = Vec::with_capacity(8 + json.len() + heights.len() * 4 + placed.len() * 32);
        out.extend_from_slice(&(json.len() as u32).to_le_bytes());
        out.extend_from_slice(&json);
        out.resize(out.len().next_multiple_of(4), 0);
        heights.iter().for_each(|h| out.extend_from_slice(&h.to_le_bytes()));
        for (index, prop) in placed {
            out.extend_from_slice(&index.to_le_bytes());
            for v in prop.position.iter().chain(&prop.rotation).chain([&prop.scale]) {
                out.extend_from_slice(&v.to_le_bytes());
            }
        }
        Ok(Response::new(out))
    })
    .await
}

/// A prop's model in its file space, without skinning:
/// `[mesh count u32]` then per mesh `[vertices u32][indices u32][flags u32: 1 normals, 2 uvs]
/// [diffuse texture file id u32, 0 = none][positions][normals][uvs][indices u32]`.
#[tauri::command]
pub async fn load_prop_model(app: AppHandle, file_id: u32) -> CmdResult<Response> {
    blocking(move || {
        let session = session(&app.state::<AppState>())?;
        let mft = session.archive.entry_for_id(file_id).ok_or_else(|| format!("No file with id {file_id}"))?;
        let data = session.archive.read(mft).map_err(err)?;
        let model = formats::model::decode_original(&data).map_err(err)?;
        // Untextured props still render.
        let materials = formats::model::materials(&data).unwrap_or_default();
        let meshes: Vec<_> = model.meshes.iter().filter(|m| !m.indices.is_empty() && !m.positions.is_empty()).collect();

        let mut out = Vec::new();
        let push_u32 = |out: &mut Vec<u8>, v: u32| out.extend_from_slice(&v.to_le_bytes());
        push_u32(&mut out, meshes.len() as u32);
        for mesh in meshes {
            let n = mesh.positions.len();
            let has_normals = mesh.normals.len() == n;
            let has_uvs = mesh.uvs.len() == n;
            let diffuse = materials
                .get(mesh.material_index as usize)
                .and_then(|m| m.texture(formats::model::TextureSlot::Diffuse))
                .filter(|_| has_uvs);
            push_u32(&mut out, n as u32);
            push_u32(&mut out, mesh.indices.len() as u32);
            push_u32(&mut out, has_normals as u32 | (has_uvs as u32) << 1);
            push_u32(&mut out, diffuse.unwrap_or(0));
            mesh.positions.iter().flatten().for_each(|v| out.extend_from_slice(&v.to_le_bytes()));
            if has_normals {
                mesh.normals.iter().flatten().for_each(|v| out.extend_from_slice(&v.to_le_bytes()));
            }
            if has_uvs {
                mesh.uvs.iter().flatten().for_each(|v| out.extend_from_slice(&v.to_le_bytes()));
            }
            mesh.indices.iter().for_each(|&i| push_u32(&mut out, i));
        }
        Ok(Response::new(out))
    })
    .await
}
