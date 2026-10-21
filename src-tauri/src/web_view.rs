//! Previews of Bink videos, Flash movies and the in-game web browser's data
//! files (resource packs, ICU data, V8 snapshots).

use gw2dat::formats::{media, web};
use serde::Serialize;
use tauri::ipc::Response;
use tauri::{AppHandle, Manager};

use crate::AppState;
use crate::commands::{CmdResult, blocking, err, session};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BinkDto {
    revision: char,
    frames: u32,
    largest_frame: u32,
    width: u32,
    height: u32,
    fps: f64,
    duration: f64,
    flags: u32,
    audio_tracks: u32,
    size: usize,
}

#[tauri::command]
pub async fn bink_info(app: AppHandle, mft: u32) -> CmdResult<BinkDto> {
    blocking(move || {
        let session = session(&app.state::<AppState>())?;
        let data = session.read(mft).map_err(err)?;
        let b = media::bink(&data).map_err(err)?;
        Ok(BinkDto {
            revision: b.revision,
            frames: b.frames,
            largest_frame: b.largest_frame,
            width: b.width,
            height: b.height,
            fps: b.fps(),
            duration: b.duration(),
            flags: b.flags,
            audio_tracks: b.audio_tracks,
            size: data.len(),
        })
    })
    .await
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FlashDto {
    compression: &'static str,
    version: u8,
    length: u32,
    /// Stage size, frame rate and frame count; missing for LZMA-compressed
    /// movies, whose header is not unpacked.
    width: Option<f32>,
    height: Option<f32>,
    frame_rate: Option<f32>,
    frames: Option<u16>,
}

#[tauri::command]
pub async fn flash_info(app: AppHandle, mft: u32) -> CmdResult<FlashDto> {
    blocking(move || {
        let session = session(&app.state::<AppState>())?;
        let f = media::flash(&session.read(mft).map_err(err)?).map_err(err)?;
        Ok(FlashDto {
            compression: f.compression,
            version: f.version,
            length: f.length,
            width: f.width,
            height: f.height,
            frame_rate: f.frame_rate,
            frames: f.frames,
        })
    })
    .await
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PakDto {
    version: u32,
    encoding: u32,
    resources: Vec<PakResourceDto>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PakResourceDto {
    id: u16,
    size: usize,
    alias_of: Option<u16>,
    /// Content kind (`png`, `css`, …), of the unpacked data when compressed.
    kind: &'static str,
    /// `gzip` or `brotli` when the resource is stored compressed.
    compression: Option<&'static str>,
}

#[tauri::command]
pub async fn pak_info(app: AppHandle, mft: u32) -> CmdResult<PakDto> {
    blocking(move || {
        let session = session(&app.state::<AppState>())?;
        let data = session.read(mft).map_err(err)?;
        let pak = web::pak(&data).map_err(err)?;
        let resources = pak
            .resources
            .iter()
            .map(|r| {
                let bytes = &data[r.offset..r.offset + r.size];
                let (kind, compression) = match web::decompress(bytes) {
                    Ok(Some(unpacked)) => (web::sniff(&unpacked), Some(web::sniff(bytes))),
                    _ => (web::sniff(bytes), None),
                };
                PakResourceDto { id: r.id, size: r.size, alias_of: r.alias_of, kind, compression }
            })
            .collect();
        Ok(PakDto { version: pak.version, encoding: pak.encoding, resources })
    })
    .await
}

/// A resource of a pack, unpacked if it is compressed.
#[tauri::command]
pub async fn pak_resource(app: AppHandle, mft: u32, id: u16) -> CmdResult<Response> {
    blocking(move || {
        let session = session(&app.state::<AppState>())?;
        let data = session.read(mft).map_err(err)?;
        let pak = web::pak(&data).map_err(err)?;
        let r = pak.resources.iter().find(|r| r.id == id).ok_or_else(|| format!("no resource {id} in the pack"))?;
        let bytes = &data[r.offset..r.offset + r.size];
        Ok(Response::new(web::decompress(bytes).map_err(err)?.unwrap_or_else(|| bytes.to_vec())))
    })
    .await
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IcuDto {
    format_version: String,
    data_version: String,
    copyright: String,
    /// (name, size).
    entries: Vec<(String, usize)>,
}

#[tauri::command]
pub async fn icu_info(app: AppHandle, mft: u32) -> CmdResult<IcuDto> {
    blocking(move || {
        let session = session(&app.state::<AppState>())?;
        let icu = web::icu(&session.read(mft).map_err(err)?).map_err(err)?;
        let dotted = |v: [u8; 4]| v.map(|n| n.to_string()).join(".");
        Ok(IcuDto {
            format_version: dotted(icu.format_version),
            data_version: dotted(icu.data_version),
            copyright: icu.copyright,
            entries: icu.entries.into_iter().map(|e| (e.name, e.size)).collect(),
        })
    })
    .await
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct V8Dto {
    contexts: u32,
    rehashable: bool,
    checksum: u32,
    version: String,
    size: usize,
}

#[tauri::command]
pub async fn v8_info(app: AppHandle, mft: u32) -> CmdResult<V8Dto> {
    blocking(move || {
        let session = session(&app.state::<AppState>())?;
        let data = session.read(mft).map_err(err)?;
        let s = web::v8_snapshot(&data).map_err(err)?;
        Ok(V8Dto {
            contexts: s.contexts,
            rehashable: s.rehashable,
            checksum: s.checksum,
            version: s.version,
            size: data.len(),
        })
    })
    .await
}
