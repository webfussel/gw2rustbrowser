//! Preview of cinematic scenes (CINP).

use gw2dat::formats::cinematic::{self, TrackGroup};
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

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SceneDto {
    version: u16,
    starting_sequence: String,
    sequences: Vec<SequenceDto>,
    texts: Vec<TextDto>,
    files: Vec<(String, Option<FileRefDto>)>,
    scripts: Vec<(String, usize)>,
    ambient_lights: Vec<LightDto>,
    species: Vec<SpeciesDto>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SequenceDto {
    name: String,
    map: String,
    client_map: String,
    length: f32,
    flags: u32,
    environment_map: Option<FileRefDto>,
    groups: Vec<GroupDto>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct GroupDto {
    name: String,
    kind: u8,
    kind_name: &'static str,
    flags: u32,
    properties: usize,
    tracks: Vec<TrackDto>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct TrackDto {
    name: String,
    kind: u8,
    /// (time, value) of the curve keys.
    curve: Vec<[f32; 2]>,
    /// (time, value) of the flag keys.
    flags: Vec<[f32; 2]>,
    triggers: Vec<TriggerDto>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct TriggerDto {
    time: f32,
    token1: String,
    token2: String,
    values: [f32; 4],
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct TextDto {
    name: String,
    index: u32,
    voice: u32,
    text: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct LightDto {
    name: String,
    /// ground, sky, fill, hemispherical: intensity and RGB.
    colors: Vec<(f32, [u8; 3])>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SpeciesDto {
    name: String,
    model: String,
    variant: String,
}

fn group(g: &TrackGroup) -> GroupDto {
    GroupDto {
        name: name(g.name),
        kind: g.kind,
        kind_name: cinematic::group_kind_name(g.kind),
        flags: g.flags,
        properties: g.properties.len(),
        tracks: g
            .tracks
            .iter()
            .map(|t| TrackDto {
                name: name(t.name),
                kind: t.kind,
                curve: t.curve_keys.iter().map(|k| [k[0], k[1]]).collect(),
                flags: t.flag_keys.clone(),
                triggers: t
                    .trigger_keys
                    .iter()
                    .map(|k| TriggerDto {
                        time: k.time,
                        token1: name(k.token1),
                        token2: name(k.token2),
                        values: k.values,
                    })
                    .collect(),
            })
            .collect(),
    }
}

#[tauri::command]
pub async fn scene_info(app: AppHandle, mft: u32) -> CmdResult<SceneDto> {
    blocking(move || {
        let session = session(&app.state::<AppState>())?;
        build(&session, &session.read(mft).map_err(err)?)
    })
    .await
}

fn build(session: &Session, data: &[u8]) -> CmdResult<SceneDto> {
    let s = cinematic::decode(data).map_err(err)?;
    {
        Ok(SceneDto {
            version: s.version,
            starting_sequence: name(s.starting_sequence),
            sequences: s
                .sequences
                .iter()
                .map(|q| SequenceDto {
                    name: name(q.name),
                    map: q.map.clone(),
                    client_map: q.client_map.clone(),
                    length: q.length,
                    flags: q.flags,
                    environment_map: file(session, q.environment_map),
                    groups: q.track_groups.iter().map(group).collect(),
                })
                .collect(),
            texts: s
                .texts
                .iter()
                .map(|t| TextDto {
                    name: name(t.name),
                    index: t.index,
                    voice: t.voice,
                    text: t.entries.iter().map(|(_, x)| x.as_str()).find(|x| !x.is_empty()).unwrap_or("").to_owned(),
                })
                .collect(),
            files: s.files.iter().map(|&(n, f)| (name(n), file(session, f))).collect(),
            scripts: s.scripts.iter().map(|&(n, size)| (name(n), size)).collect(),
            ambient_lights: s
                .ambient_lights
                .iter()
                .map(|l| LightDto { name: name(l.name), colors: l.colors.to_vec() })
                .collect(),
            species: s
                .species
                .iter()
                .map(|sp| SpeciesDto { name: name(sp.name), model: name(sp.model), variant: name(sp.variant) })
                .collect(),
        })
    }
}
