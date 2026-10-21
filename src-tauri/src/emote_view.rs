//! Emote animation (emoc) preview.

use gw2dat::formats::{emotes, material::token_label};
use serde::Serialize;
use tauri::{AppHandle, Manager};

use crate::AppState;
use crate::commands::{CmdResult, blocking, err, session};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EmoteDto {
    name: String,
    timings: Vec<TimingDto>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct TimingDto {
    file_id: Option<u32>,
    /// MFT index of the model file, -1 if it is missing.
    mft: i64,
    /// File type name of the model, e.g. `MODL animset`.
    file_type: Option<&'static str>,
    blend_in: f32,
    blend_out: f32,
    intro: u32,
    loop_length: u32,
    outro: u32,
    start_offset: u32,
}

#[tauri::command]
pub async fn emote_info(app: AppHandle, mft: u32) -> CmdResult<Vec<EmoteDto>> {
    blocking(move || {
        let session = session(&app.state::<AppState>())?;
        let list = emotes::decode(&session.read(mft).map_err(err)?).map_err(err)?;
        Ok(list
            .iter()
            .map(|e| EmoteDto {
                name: token_label(e.name),
                timings: e
                    .timings
                    .iter()
                    .map(|t| {
                        let mft = t.model.and_then(|id| session.archive.entry_for_id(id));
                        TimingDto {
                            file_id: t.model,
                            mft: mft.map_or(-1, i64::from),
                            file_type: mft.and_then(|m| session.entry(m)).map(|e| e.file_type.name()),
                            blend_in: t.blend_in,
                            blend_out: t.blend_out,
                            intro: t.intro,
                            loop_length: t.loop_length,
                            outro: t.outro,
                            start_offset: t.start_offset,
                        }
                    })
                    .collect(),
            })
            .collect())
    })
    .await
}
