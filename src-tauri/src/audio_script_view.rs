//! Audio script (AMSP) preview.

use gw2dat::formats::audio_script::{self, Param, Range, SoundFile};
use gw2dat::formats::material::token_label;
use serde::Serialize;
use tauri::{AppHandle, Manager};

use crate::AppState;
use crate::commands::{CmdResult, blocking, err, session};
use crate::session::Session;

fn name(token: u64) -> Option<String> {
    (token != 0).then(|| token_label(token))
}

fn param_text(p: &Param) -> String {
    let mut text = match p.random {
        Some((lo, hi)) if lo != hi => format!("{} – {}", round(lo), round(hi)),
        _ => round(p.value),
    };
    if let Some(e) = &p.envelope {
        text.push_str(&format!(" · envelope on {} ({} points)", name(e.input).unwrap_or("?".into()), e.points.len()));
    }
    text
}

fn round(v: f32) -> String {
    let text = format!("{v:.3}");
    text.trim_end_matches('0').trim_end_matches('.').to_owned()
}

fn range_text(r: &Range) -> Option<String> {
    match (r.min, r.max) {
        (0.0, 0.0) => None,
        (lo, hi) if lo == hi => Some(round(lo)),
        (lo, hi) => Some(format!("{} – {}", round(lo), round(hi))),
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct FileDto {
    id: u32,
    /// MFT index, -1 if the file is not in the archive.
    mft: i64,
    /// File type name, e.g. `PackedMP3`.
    kind: Option<&'static str>,
}

fn file(session: &Session, id: Option<u32>) -> Option<FileDto> {
    let id = id?;
    let mft = session.archive.entry_for_id(id);
    Some(FileDto {
        id,
        mft: mft.map_or(-1, i64::from),
        kind: mft.and_then(|m| session.entry(m)).map(|e| e.file_type.name()),
    })
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SoundFileDto {
    file: Option<FileDto>,
    volume: f32,
    weight: f32,
    condition: Option<String>,
    language: Option<String>,
}

fn sound_files(session: &Session, files: &[SoundFile]) -> Vec<SoundFileDto> {
    files
        .iter()
        .map(|f| SoundFileDto {
            file: file(session, f.file),
            volume: f.volume,
            weight: f.weight,
            condition: name(f.condition),
            language: name(f.language),
        })
        .collect()
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SoundDto {
    name: String,
    category: Option<String>,
    files: Vec<SoundFileDto>,
    volume: String,
    pitch: String,
    pan: String,
    fade_in: f32,
    fade_out: f32,
    loop_count: u32,
    flags: u32,
    channel_max: u32,
    priority: u8,
    playback_mode: u8,
    position_mode: u8,
    file_iterate_mode: u8,
    loop_mode: u8,
    dsp_effects: usize,
    attenuation: bool,
    initial_delay: Option<String>,
    repeat_count: Option<String>,
    repeat_time: Option<String>,
    play_length: Option<String>,
    end_cue: Option<String>,
    offset_bone: Option<String>,
    playlist: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PlaylistDto {
    name: String,
    category: Option<String>,
    primary: Option<String>,
    secondary: Option<String>,
    files: Vec<SoundFileDto>,
    volume: String,
    fade_in: f32,
    fade_out: f32,
    flags: u32,
    file_iterate_mode: u8,
    initial_silence: Option<String>,
    interval_silence: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SettingsDto {
    default_snapshot: Option<String>,
    distance_scale: f32,
    doppler_scale: f32,
    busses: usize,
    categories: usize,
    materials: usize,
    reverbs: usize,
    snapshots: usize,
    playlists: Vec<PlaylistDto>,
    /// (name, external playlist).
    music_external: Vec<(String, String)>,
    bank_index: Option<FileDto>,
    bank_script: Option<FileDto>,
    music_script: Option<FileDto>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct TriggerDto {
    name: String,
    markers: Vec<MarkerDto>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct MarkerDto {
    time: f32,
    cue: Option<String>,
    end: Option<String>,
    kind: u8,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct HandlerDto {
    name: String,
    flags: u32,
    size: usize,
    /// Byte code as hex (at most 1 KiB shown).
    code: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct NamedFileDto {
    name: String,
    file: Option<FileDto>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PropertyDto {
    name: String,
    token: Option<String>,
    value: f32,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioScriptDto {
    version: u16,
    volume: f32,
    fade_in: f32,
    flags: u32,
    sound_pool_count: u32,
    sound_pool_delay: f32,
    music_cue: Option<String>,
    reverb_override: Option<String>,
    snapshot: Option<String>,
    sounds: Vec<SoundDto>,
    triggers: Vec<TriggerDto>,
    handlers: Vec<HandlerDto>,
    script_refs: Vec<NamedFileDto>,
    properties: Vec<PropertyDto>,
    settings: Option<SettingsDto>,
}

#[tauri::command]
pub async fn audio_script_info(app: AppHandle, mft: u32) -> CmdResult<AudioScriptDto> {
    blocking(move || {
        let session = session(&app.state::<AppState>())?;
        let data = session.read(mft).map_err(err)?;
        build(&session, &data)
    })
    .await
}

fn build(session: &Session, data: &[u8]) -> CmdResult<AudioScriptDto> {
    let s = audio_script::decode(data).map_err(err)?;
    Ok(AudioScriptDto {
        version: s.version,
        volume: s.volume,
        fade_in: s.fade_in,
        flags: s.flags,
        sound_pool_count: s.sound_pool_count,
        sound_pool_delay: s.sound_pool_delay,
        music_cue: name(s.music_cue),
        reverb_override: name(s.reverb_override),
        snapshot: name(s.snapshot),
        sounds: s
            .sounds
            .iter()
            .map(|m| SoundDto {
                name: token_label(m.name),
                category: name(m.category),
                files: sound_files(session, &m.files),
                volume: param_text(&m.volume),
                pitch: param_text(&m.pitch),
                pan: param_text(&m.pan),
                fade_in: m.fade_in,
                fade_out: m.fade_out,
                loop_count: m.loop_count,
                flags: m.flags,
                channel_max: m.channel_max,
                priority: m.channel_priority,
                playback_mode: m.playback_mode,
                position_mode: m.position_mode,
                file_iterate_mode: m.file_iterate_mode,
                loop_mode: m.loop_mode,
                dsp_effects: m.dsp_effects,
                attenuation: m.attenuation,
                initial_delay: range_text(&m.initial_delay),
                repeat_count: range_text(&m.repeat_count),
                repeat_time: range_text(&m.repeat_time),
                play_length: range_text(&m.play_length),
                end_cue: name(m.end_cue),
                offset_bone: name(m.offset_bone),
                playlist: name(m.playlist),
            })
            .collect(),
        triggers: s
            .triggers
            .iter()
            .map(|t| TriggerDto {
                name: token_label(t.name),
                markers: t
                    .markers
                    .iter()
                    .map(|m| MarkerDto { time: m.time, cue: name(m.cue), end: name(m.end), kind: m.kind })
                    .collect(),
            })
            .collect(),
        handlers: s
            .handlers
            .iter()
            .map(|h| HandlerDto {
                name: token_label(h.name),
                flags: h.flags,
                size: h.code.len(),
                code: h.code.iter().take(1024).map(|b| format!("{b:02x}")).collect::<Vec<_>>().join(" "),
            })
            .collect(),
        script_refs: s
            .script_refs
            .iter()
            .map(|(n, f)| NamedFileDto { name: token_label(*n), file: file(session, *f) })
            .collect(),
        properties: s
            .properties
            .iter()
            .map(|p| PropertyDto { name: token_label(p.name), token: name(p.token), value: p.value })
            .collect(),
        settings: s.settings.as_ref().map(|st| SettingsDto {
            default_snapshot: name(st.default_snapshot),
            distance_scale: st.distance_scale,
            doppler_scale: st.doppler_scale,
            busses: st.busses,
            categories: st.categories,
            materials: st.materials,
            reverbs: st.reverbs,
            snapshots: st.snapshots,
            playlists: st
                .playlists
                .iter()
                .map(|p| PlaylistDto {
                    name: token_label(p.name),
                    category: name(p.category),
                    primary: name(p.primary),
                    secondary: name(p.secondary),
                    files: sound_files(session, &p.files),
                    volume: param_text(&p.volume),
                    fade_in: p.fade_in,
                    fade_out: p.fade_out,
                    flags: p.flags,
                    file_iterate_mode: p.file_iterate_mode,
                    initial_silence: range_text(&p.initial_silence),
                    interval_silence: range_text(&p.interval_silence),
                })
                .collect(),
            music_external: st.music_external.iter().map(|(n, w)| (token_label(*n), w.clone())).collect(),
            bank_index: file(session, st.bank_index),
            bank_script: file(session, st.bank_script),
            music_script: file(session, st.music_script),
        }),
    })
}
