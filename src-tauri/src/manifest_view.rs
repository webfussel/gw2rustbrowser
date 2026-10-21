//! Previews of the model collision manifest (cmaC) and portal manifests (prlt).

use std::collections::HashMap;

use gw2dat::formats::manifests;
use serde::Serialize;
use tauri::{AppHandle, Manager};

use crate::AppState;
use crate::commands::{CmdResult, blocking, err, session};
use crate::session::Session;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileRefDto {
    id: u32,
    /// MFT index, -1 if the file is not in the archive.
    mft: i64,
    /// File type name, e.g. `MODL static`.
    kind: Option<&'static str>,
}

fn file_ref(session: &Session, id: Option<u32>) -> Option<FileRefDto> {
    let id = id?;
    let mft = session.archive.entry_for_id(id);
    Some(FileRefDto {
        id,
        mft: mft.map_or(-1, i64::from),
        kind: mft.and_then(|m| session.entry(m)).map(|e| e.file_type.name()),
    })
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CollisionEntryDto {
    name: String,
    model: Option<FileRefDto>,
    collision: Option<FileRefDto>,
    scales: Vec<f32>,
}

#[tauri::command]
pub async fn collision_manifest_info(app: AppHandle, mft: u32) -> CmdResult<Vec<CollisionEntryDto>> {
    blocking(move || {
        let session = session(&app.state::<AppState>())?;
        let entries = manifests::collision_manifest(&session.read(mft).map_err(err)?).map_err(err)?;
        Ok(entries
            .into_iter()
            .map(|e| CollisionEntryDto {
                name: e.name,
                model: file_ref(&session, e.model),
                collision: file_ref(&session, e.collision),
                scales: e.scales,
            })
            .collect())
    })
    .await
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PortalMapDto {
    guid: String,
    redirectors: Vec<RedirectorDto>,
    starts: Vec<StartDto>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct RedirectorDto {
    /// Target map.
    guid: String,
    token: u32,
    position: [f32; 3],
    radius: f32,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct StartDto {
    token: u32,
    position: [f32; 3],
    radius: f32,
    models: usize,
    /// Models per file type name (`missing` for files not in the archive).
    kinds: HashMap<&'static str, usize>,
}

#[tauri::command]
pub async fn portal_manifest_info(app: AppHandle, mft: u32) -> CmdResult<Vec<PortalMapDto>> {
    blocking(move || {
        let session = session(&app.state::<AppState>())?;
        let maps = manifests::portal_manifest(&session.read(mft).map_err(err)?).map_err(err)?;
        Ok(maps
            .iter()
            .map(|m| PortalMapDto {
                guid: manifests::guid_text(&m.guid),
                redirectors: m
                    .redirectors
                    .iter()
                    .map(|r| RedirectorDto {
                        guid: manifests::guid_text(&r.guid),
                        token: r.token,
                        position: r.position,
                        radius: r.radius,
                    })
                    .collect(),
                starts: m
                    .starts
                    .iter()
                    .map(|s| {
                        let mut kinds = HashMap::new();
                        for model in &s.models {
                            let kind = model
                                .file
                                .and_then(|id| session.archive.entry_for_id(id))
                                .and_then(|m| session.entry(m))
                                .map_or("missing", |e| e.file_type.name());
                            *kinds.entry(kind).or_default() += 1;
                        }
                        StartDto {
                            token: s.token,
                            position: s.position,
                            radius: s.radius,
                            models: s.models.len(),
                            kinds,
                        }
                    })
                    .collect(),
            })
            .collect())
    })
    .await
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StartModelDto {
    file: Option<FileRefDto>,
    flags: u32,
    kind: u32,
    permutation: String,
}

/// The models of one start point (lists can have tens of thousands).
#[tauri::command]
pub async fn portal_start_models(app: AppHandle, mft: u32, map: usize, start: usize) -> CmdResult<Vec<StartModelDto>> {
    blocking(move || {
        let session = session(&app.state::<AppState>())?;
        let maps = manifests::portal_manifest(&session.read(mft).map_err(err)?).map_err(err)?;
        let start = maps.get(map).and_then(|m| m.starts.get(start)).ok_or("No such start point")?;
        Ok(start
            .models
            .iter()
            .map(|m| StartModelDto {
                file: file_ref(&session, m.file),
                flags: m.flags,
                kind: m.kind,
                permutation: format!("{:#x}", m.permutation),
            })
            .collect())
    })
    .await
}
