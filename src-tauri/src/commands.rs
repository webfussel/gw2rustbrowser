//! Commands invoked from the frontend.

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use gw2dat::{Archive, Index, IndexEntry, formats};
use serde::Serialize;
use tauri::ipc::Response;
use tauri::{AppHandle, Emitter, Manager, State};

use crate::session::Session;
use crate::{AppState, settings};

pub type CmdResult<T> = Result<T, String>;

pub fn session(state: &AppState) -> CmdResult<Arc<Session>> {
    state.session.read().unwrap().clone().ok_or_else(|| "No archive is open".to_string())
}

/// Runs blocking work off the async runtime.
pub async fn blocking<T: Send + 'static>(work: impl FnOnce() -> CmdResult<T> + Send + 'static) -> CmdResult<T> {
    tauri::async_runtime::spawn_blocking(work).await.map_err(|e| e.to_string())?
}

pub fn err(e: impl std::fmt::Display) -> String {
    e.to_string()
}

/// The archive to open on start: a command line argument, else the last one.
#[tauri::command]
pub fn startup_path(app: AppHandle) -> Option<String> {
    let from_args = std::env::args().skip(1).find(|a| !a.starts_with('-'));
    from_args.or_else(|| settings::last_dat(&app)).filter(|p| std::path::Path::new(p).is_file())
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ScanEvent {
    done: u32,
    total: u32,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DatSummary {
    path: String,
    files: u32,
    categories: u32,
    rescanned: u32,
    from_cache: bool,
    millis: u64,
}

#[tauri::command]
pub async fn open_dat(app: AppHandle, path: String) -> CmdResult<DatSummary> {
    blocking(move || {
        let started = Instant::now();
        let state = app.state::<AppState>();
        let archive = Archive::open(&path).map_err(err)?;
        let cache = settings::index_cache(&app, archive.path());
        let previous = cache.as_deref().and_then(Index::load);

        // Report scan progress while the scan runs on the rayon pool.
        let progress = state.scan.clone();
        progress.done.store(0, Ordering::Relaxed);
        progress.total.store(0, Ordering::Relaxed);
        let finished = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let reporter = {
            let (app, progress, finished) = (app.clone(), progress.clone(), finished.clone());
            std::thread::spawn(move || {
                while !finished.load(Ordering::Relaxed) {
                    let event = ScanEvent {
                        done: progress.done.load(Ordering::Relaxed),
                        total: progress.total.load(Ordering::Relaxed),
                    };
                    let _ = app.emit("scan-progress", event);
                    std::thread::sleep(Duration::from_millis(120));
                }
            })
        };
        let index = Index::update(&archive, previous.as_ref(), &progress);
        finished.store(true, Ordering::Relaxed);
        let _ = reporter.join();

        let rescanned = progress.total.load(Ordering::Relaxed);
        if let Some(cache) = &cache
            && (previous.is_none() || rescanned > 0)
        {
            let _ = index.save(cache);
        }
        settings::set_last_dat(&app, archive.path());

        let summary = DatSummary {
            path: archive.path().to_string_lossy().into_owned(),
            files: index.entries.len() as u32,
            categories: index.categories.len() as u32,
            rescanned,
            from_cache: previous.is_some(),
            millis: started.elapsed().as_millis() as u64,
        };
        *state.session.write().unwrap() = Some(Arc::new(Session::new(archive, index)));
        Ok(summary)
    })
    .await
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CategoryDto {
    id: u32,
    name: String,
    parent: Option<u32>,
    /// Files in this category and all subcategories.
    total: u32,
    /// Files directly in this category.
    files: u32,
    /// Representative file type below this category, for its icon.
    content: Option<&'static str>,
    /// Kind of that type (lower case).
    content_kind: Option<String>,
    /// Whether the category contains several file types.
    mixed: bool,
}

#[tauri::command]
pub fn categories(state: State<'_, AppState>) -> CmdResult<Vec<CategoryDto>> {
    let session = session(&state)?;
    Ok(session
        .index
        .categories
        .iter()
        .enumerate()
        .map(|(id, c)| CategoryDto {
            id: id as u32,
            name: c.name.clone(),
            parent: c.parent,
            total: session.totals[id],
            files: session.files[id].len() as u32,
            content: session.content[id].map(|(t, _)| t.name()),
            content_kind: session.content[id].map(|(t, _)| format!("{:?}", t.kind()).to_lowercase()),
            mixed: session.content[id].is_some_and(|(_, mixed)| mixed),
        })
        .collect())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileDto {
    mft: u32,
    base_id: u32,
    file_id: u32,
    #[serde(rename = "type")]
    file_type: &'static str,
    size: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    known_name: Option<&'static str>,
}

impl From<&IndexEntry> for FileDto {
    fn from(e: &IndexEntry) -> Self {
        FileDto {
            mft: e.mft_index,
            base_id: e.base_id,
            file_id: e.file_id,
            file_type: e.file_type.name(),
            size: e.size,
            known_name: e.known_name(),
        }
    }
}

#[tauri::command]
pub fn category_files(state: State<'_, AppState>, category: u32) -> CmdResult<Vec<FileDto>> {
    let session = session(&state)?;
    let list = session.files.get(category as usize).ok_or("Unknown category")?;
    Ok(list.iter().map(|&i| FileDto::from(&session.index.entries[i as usize])).collect())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EntryDetail {
    #[serde(flatten)]
    file: FileDto,
    kind: String,
    category: u32,
    path: Vec<String>,
    stored_size: u32,
    offset: u64,
    compressed: bool,
    converted_extension: &'static str,
    raw_extension: &'static str,
}

#[tauri::command]
pub fn entry_info(state: State<'_, AppState>, mft: u32) -> CmdResult<EntryDetail> {
    let session = session(&state)?;
    let entry = session.entry(mft).ok_or("Unknown file")?;
    let stored = session.archive.entry(mft).map_err(err)?;
    Ok(EntryDetail {
        file: FileDto::from(entry),
        kind: format!("{:?}", entry.file_type.kind()).to_lowercase(),
        category: entry.category,
        path: session.index.category_path(entry.category),
        stored_size: stored.size,
        offset: stored.offset,
        compressed: stored.is_compressed(),
        converted_extension: formats::converted_extension(entry.file_type),
        raw_extension: entry.file_type.extension(),
    })
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FindResult {
    #[serde(flatten)]
    file: FileDto,
    category: u32,
    path: Vec<String>,
}

/// Finds files by id: exact base/file id matches first, then base ids that
/// start with the query. `#123` looks up an MFT index.
#[tauri::command]
pub fn find(state: State<'_, AppState>, query: String) -> CmdResult<Vec<FindResult>> {
    const LIMIT: usize = 200;
    let session = session(&state)?;
    let query = query.trim();
    let result = |e: &IndexEntry| FindResult {
        file: FileDto::from(e),
        category: e.category,
        path: session.index.category_path(e.category),
    };

    if let Some(mft) = query.strip_prefix('#') {
        let mft: u32 = mft.parse().map_err(|_| "Not a number")?;
        return Ok(session.entry(mft).map(result).into_iter().collect());
    }
    let Ok(id) = query.parse::<u32>() else {
        return Ok(Vec::new());
    };

    let mut found: Vec<FindResult> = Vec::new();
    let mut seen = std::collections::HashSet::new();
    if let Some(e) = session.archive.entry_for_id(id).and_then(|mft| session.entry(mft)) {
        seen.insert(e.mft_index);
        found.push(result(e));
    }
    for e in &session.index.entries {
        if found.len() >= LIMIT {
            break;
        }
        if e.base_id != 0 && !seen.contains(&e.mft_index) && e.base_id.to_string().starts_with(query) {
            seen.insert(e.mft_index);
            found.push(result(e));
        }
    }
    Ok(found)
}

/// Decompressed file content as binary.
#[tauri::command]
pub async fn read_file(app: AppHandle, mft: u32) -> CmdResult<Response> {
    blocking(move || {
        let session = session(&app.state::<AppState>())?;
        Ok(Response::new(session.read(mft).map_err(err)?.to_vec()))
    })
    .await
}

/// Image as `[width u32][height u32][format length u16][format][RGBA…]`.
fn image_response(image: formats::image::RgbaImage) -> Response {
    let mut out = Vec::with_capacity(10 + image.format.len() + image.pixels.len());
    out.extend_from_slice(&image.width.to_le_bytes());
    out.extend_from_slice(&image.height.to_le_bytes());
    out.extend_from_slice(&(image.format.len() as u16).to_le_bytes());
    out.extend_from_slice(image.format.as_bytes());
    out.extend_from_slice(&image.pixels);
    Response::new(out)
}

#[tauri::command]
pub async fn load_image(app: AppHandle, mft: u32) -> CmdResult<Response> {
    blocking(move || {
        let session = session(&app.state::<AppState>())?;
        let data = session.read(mft).map_err(err)?;
        Ok(image_response(formats::image::decode(&data).map_err(err)?))
    })
    .await
}

/// Image by file id (textures referenced from models). Does not touch the
/// "recent file" cache so the selected file stays cached.
#[tauri::command]
pub async fn load_texture(app: AppHandle, file_id: u32) -> CmdResult<Response> {
    blocking(move || {
        let session = session(&app.state::<AppState>())?;
        let mft = session.archive.entry_for_id(file_id).ok_or_else(|| format!("No file with id {file_id}"))?;
        let data = session.archive.read(mft).map_err(err)?;
        Ok(image_response(formats::image::decode(&data).map_err(err)?))
    })
    .await
}

/// Sound stream as `[mime length u8][mime][stream…]`.
fn sound_response(sound: formats::audio::Sound) -> Response {
    let mime = sound.codec.mime().as_bytes();
    let mut out = Vec::with_capacity(1 + mime.len() + sound.data.len());
    out.push(mime.len() as u8);
    out.extend_from_slice(mime);
    out.extend_from_slice(&sound.data);
    Response::new(out)
}

#[tauri::command]
pub async fn load_sound(app: AppHandle, mft: u32) -> CmdResult<Response> {
    blocking(move || {
        let session = session(&app.state::<AppState>())?;
        let data = session.read(mft).map_err(err)?;
        Ok(sound_response(formats::audio::decode(&data).map_err(err)?))
    })
    .await
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BankEntryDto {
    index: usize,
    voice_id: u32,
    codec: Option<&'static str>,
    size: usize,
    length: Option<f32>,
    error: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BankIndexLanguageDto {
    code: usize,
    name: String,
    /// Bank file id per bank number, 0 for none.
    files: Vec<u32>,
    /// MFT index of each bank file, -1 if it is missing.
    mfts: Vec<i64>,
}

/// The languages of a sound bank index that have banks.
#[tauri::command]
pub async fn load_bank_index(app: AppHandle, mft: u32) -> CmdResult<Vec<BankIndexLanguageDto>> {
    blocking(move || {
        let session = session(&app.state::<AppState>())?;
        let data = session.read(mft).map_err(err)?;
        let index = formats::audio::decode_bank_index(&data).map_err(err)?;
        Ok(index
            .languages
            .iter()
            .enumerate()
            .filter(|(_, banks)| banks.iter().any(Option::is_some))
            .map(|(code, banks)| BankIndexLanguageDto {
                code,
                name: match formats::text::language_name(code as u8) {
                    "Unknown" => format!("Language {code}"),
                    name => name.to_owned(),
                },
                files: banks.iter().map(|f| f.unwrap_or(0)).collect(),
                mfts: banks
                    .iter()
                    .map(|f| f.and_then(|id| session.archive.entry_for_id(id)).map_or(-1, i64::from))
                    .collect(),
            })
            .collect())
    })
    .await
}

#[tauri::command]
pub async fn load_bank(app: AppHandle, mft: u32) -> CmdResult<Vec<BankEntryDto>> {
    blocking(move || {
        let session = session(&app.state::<AppState>())?;
        let data = session.read(mft).map_err(err)?;
        let entries = formats::audio::decode_bank(&data).map_err(err)?;
        Ok(entries
            .into_iter()
            .map(|e| match e.sound {
                Ok(sound) => BankEntryDto {
                    index: e.index,
                    voice_id: e.voice_id,
                    codec: Some(sound.codec.extension()),
                    size: sound.data.len(),
                    length: sound.length_seconds,
                    error: None,
                },
                Err(error) => BankEntryDto {
                    index: e.index,
                    voice_id: e.voice_id,
                    codec: None,
                    size: 0,
                    length: None,
                    error: Some(error.to_string()),
                },
            })
            .collect())
    })
    .await
}

#[tauri::command]
pub async fn load_bank_sound(app: AppHandle, mft: u32, index: usize) -> CmdResult<Response> {
    blocking(move || {
        let session = session(&app.state::<AppState>())?;
        let data = session.read(mft).map_err(err)?;
        let entry = formats::audio::decode_bank(&data)
            .map_err(err)?
            .into_iter()
            .find(|e| e.index == index)
            .ok_or("No such bank entry")?;
        Ok(sound_response(entry.sound.map_err(err)?))
    })
    .await
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StringsDto {
    language: &'static str,
    /// `[index, text]`; encrypted entries have no text.
    entries: Vec<(u32, Option<String>)>,
}

#[tauri::command]
pub async fn load_strings(app: AppHandle, mft: u32) -> CmdResult<StringsDto> {
    blocking(move || {
        let session = session(&app.state::<AppState>())?;
        let data = session.read(mft).map_err(err)?;
        let file = formats::text::decode_strings(&data).map_err(err)?;
        Ok(StringsDto {
            language: formats::text::language_name(file.language),
            entries: file.entries.into_iter().map(|e| (e.index, e.text)).collect(),
        })
    })
    .await
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EulaDto {
    language: &'static str,
    text: String,
}

#[tauri::command]
pub async fn load_eula(app: AppHandle, mft: u32) -> CmdResult<Vec<EulaDto>> {
    blocking(move || {
        let session = session(&app.state::<AppState>())?;
        let data = session.read(mft).map_err(err)?;
        let texts = formats::text::decode_eula(&data).map_err(err)?;
        Ok(texts
            .into_iter()
            .map(|t| EulaDto { language: formats::text::language_name(t.language), text: t.text })
            .collect())
    })
    .await
}

/// Model geometry and skinning in the binary form of `rig::model_binary`.
#[tauri::command]
pub async fn load_model(app: AppHandle, mft: u32) -> CmdResult<Response> {
    blocking(move || {
        let session = session(&app.state::<AppState>())?;
        let data = session.read(mft).map_err(err)?;
        Ok(Response::new(crate::rig::model_binary(&session, &data)?))
    })
    .await
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AnimationDto {
    name: String,
    duration: f32,
    time_step: f32,
    frames: u32,
    oversampling: f32,
    track_groups: u32,
    pointer_bits: u32,
    chunk_version: u16,
    data_size: usize,
    relocations: usize,
}

#[tauri::command]
pub async fn load_animation(app: AppHandle, mft: u32) -> CmdResult<AnimationDto> {
    blocking(move || {
        let session = session(&app.state::<AppState>())?;
        let data = session.read(mft).map_err(err)?;
        let info = formats::animation::info(&data).map_err(err)?;
        Ok(AnimationDto {
            frames: info.frames(),
            name: info.name,
            duration: info.duration,
            time_step: info.time_step,
            oversampling: info.oversampling,
            track_groups: info.track_groups,
            pointer_bits: if info.wide_pointers { 64 } else { 32 },
            chunk_version: info.chunk_version,
            data_size: info.data_size,
            relocations: info.relocations,
        })
    })
    .await
}

#[tauri::command]
pub async fn model_rig(app: AppHandle, mft: u32) -> CmdResult<crate::rig::RigDto> {
    blocking(move || {
        let session = session(&app.state::<AppState>())?;
        crate::rig::rig(&session, mft)
    })
    .await
}

#[tauri::command]
pub async fn model_clip(app: AppHandle, source: u32, index: usize) -> CmdResult<crate::rig::ClipDto> {
    blocking(move || {
        let session = session(&app.state::<AppState>())?;
        crate::rig::clip(&session, source, index)
    })
    .await
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelSummaryDto {
    bones: usize,
    meshes: u32,
    animations: usize,
    materials: usize,
    /// External skeleton this model is bound to.
    skeleton: Option<u32>,
    /// Models bound to this file as their skeleton.
    users: u32,
}

#[tauri::command]
pub async fn model_summary(app: AppHandle, mft: u32) -> CmdResult<ModelSummaryDto> {
    blocking(move || {
        let session = session(&app.state::<AppState>())?;
        let entry = *session.entry(mft).ok_or("Unknown file")?;
        let data = session.read(mft).map_err(err)?;
        let summary = formats::model::summary(&data).map_err(err)?;
        let users = [entry.base_id, entry.file_id]
            .iter()
            .filter(|&&id| id != 0)
            .map(|id| session.skeleton_users.get(id).copied().unwrap_or(0))
            .sum();
        Ok(ModelSummaryDto {
            bones: summary.info.bones,
            meshes: summary.info.meshes,
            animations: summary.animations,
            materials: summary.materials,
            skeleton: summary.info.skeleton,
            users,
        })
    })
    .await
}

/// Text rendering of a file: plain text as-is, game content as XML.
#[tauri::command]
pub async fn load_text(app: AppHandle, mft: u32) -> CmdResult<String> {
    blocking(move || {
        let session = session(&app.state::<AppState>())?;
        let entry = session.entry(mft).ok_or("Unknown file")?;
        let data = session.read(mft).map_err(err)?;
        match entry.file_type.kind() {
            gw2dat::Kind::Content => formats::content::to_xml(&data).map_err(err),
            _ => Ok(decode_text(&data)),
        }
    })
    .await
}

/// Decodes text with an optional UTF-8/UTF-16 byte order mark.
fn decode_text(bytes: &[u8]) -> String {
    let utf16 = |rest: &[u8], le: bool| {
        let units: Vec<u16> = rest
            .as_chunks::<2>()
            .0
            .iter()
            .map(|b| if le { u16::from_le_bytes([b[0], b[1]]) } else { u16::from_be_bytes([b[0], b[1]]) })
            .collect();
        String::from_utf16_lossy(&units)
    };
    match bytes {
        [0xff, 0xfe, rest @ ..] => utf16(rest, true),
        [0xfe, 0xff, rest @ ..] => utf16(rest, false),
        [0xef, 0xbb, 0xbf, rest @ ..] => String::from_utf8_lossy(rest).into_owned(),
        _ => String::from_utf8_lossy(bytes).into_owned(),
    }
}

#[tauri::command]
pub async fn textpack_info(app: AppHandle) -> CmdResult<crate::texts::TextPackInfo> {
    blocking(move || {
        let session = session(&app.state::<AppState>())?;
        crate::texts::info(&session.texts, &session)
    })
    .await
}

/// Texts with ids `start..start + count`.
#[tauri::command]
pub async fn text_rows(
    app: AppHandle,
    language: usize,
    start: u32,
    count: u32,
) -> CmdResult<Vec<crate::texts::TextRow>> {
    blocking(move || {
        let session = session(&app.state::<AppState>())?;
        let texts = session.texts.texts(&session)?;
        let voices = session.texts.voice_index();
        let end = start.saturating_add(count.min(1000)).min(texts.manifest.text_count());
        Ok((start..end).map(|id| crate::texts::row(&session, &texts, voices.as_deref(), language, id)).collect())
    })
    .await
}

/// Texts with the given ids (search results).
#[tauri::command]
pub async fn search_texts(app: AppHandle, language: usize, query: String) -> CmdResult<Vec<crate::texts::TextRow>> {
    blocking(move || {
        let session = session(&app.state::<AppState>())?;
        let texts = session.texts.texts(&session)?;
        let voices = session.texts.voice_index();
        let ids = crate::texts::search(&session, &texts, language, &query, 2000);
        Ok(ids.into_iter().map(|id| crate::texts::row(&session, &texts, voices.as_deref(), language, id)).collect())
    })
    .await
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VariantRow {
    #[serde(flatten)]
    text: crate::texts::TextRow,
    variants: Vec<crate::texts::TextRow>,
}

#[tauri::command]
pub async fn variant_rows(app: AppHandle, language: usize, start: usize, count: usize) -> CmdResult<Vec<VariantRow>> {
    blocking(move || {
        let session = session(&app.state::<AppState>())?;
        let texts = session.texts.texts(&session)?;
        let voices = session.texts.voice_index();
        let row = |id| crate::texts::row(&session, &texts, voices.as_deref(), language, id);
        Ok(texts
            .variants
            .iter()
            .skip(start)
            .take(count.min(500))
            .map(|(id, variants)| {
                // The lists repeat their entries; show each variant once.
                let mut unique = variants.clone();
                unique.dedup();
                let mut seen = std::collections::HashSet::new();
                unique.retain(|v| seen.insert(*v));
                VariantRow { text: row(*id), variants: unique.into_iter().map(row).collect() }
            })
            .collect())
    })
    .await
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VoicePage {
    /// Number of voice lines matching the filter.
    total: usize,
    rows: Vec<crate::texts::TextRow>,
}

/// Voice lines `start..start + count` of the voices table, optionally only
/// those with playable audio (once the voice index is built).
#[tauri::command]
pub async fn voice_rows(
    app: AppHandle,
    language: usize,
    start: usize,
    count: usize,
    playable_only: bool,
) -> CmdResult<VoicePage> {
    blocking(move || {
        let session = session(&app.state::<AppState>())?;
        let texts = session.texts.texts(&session)?;
        let voices = session.texts.voice_index();
        let lines: Vec<u32> = match (&voices, playable_only) {
            (Some(index), true) => {
                texts.voices.iter().filter(|(_, v)| index.get(v).is_some_and(|e| e.2)).map(|(id, _)| *id).collect()
            }
            _ => texts.voices.iter().map(|(id, _)| *id).collect(),
        };
        let rows = lines
            .iter()
            .skip(start)
            .take(count.min(1000))
            .map(|id| crate::texts::row(&session, &texts, voices.as_deref(), language, *id))
            .collect();
        Ok(VoicePage { total: lines.len(), rows })
    })
    .await
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VoiceIndexDto {
    entries: usize,
    playable: usize,
}

/// Builds the voice id -> sound bank index (cached on disk).
#[tauri::command]
pub async fn build_voice_index(app: AppHandle) -> CmdResult<VoiceIndexDto> {
    blocking(move || {
        let session = session(&app.state::<AppState>())?;
        let cache = settings::voice_cache(&app, session.archive.path());
        let index = session.texts.ensure_voice_index(&session, cache.as_deref());
        Ok(VoiceIndexDto { entries: index.len(), playable: index.values().filter(|e| e.2).count() })
    })
    .await
}

#[tauri::command]
pub async fn play_voice(app: AppHandle, voice: u32) -> CmdResult<Response> {
    blocking(move || {
        let session = session(&app.state::<AppState>())?;
        let index = session.texts.voice_index().ok_or("The voice index is not built yet")?;
        let &(mft, entry, _) = index.get(&voice).ok_or("No sound bank has this voice line")?;
        let data = session.archive.read(mft).map_err(err)?;
        let found = formats::audio::decode_bank(&data)
            .map_err(err)?
            .into_iter()
            .find(|e| e.index == entry)
            .ok_or("Sound bank entry missing")?;
        Ok(sound_response(found.sound.map_err(err)?))
    })
    .await
}

/// The animation sequence table as JSON (same form as the export).
#[tauri::command]
pub async fn load_sequences(app: AppHandle, mft: u32) -> CmdResult<Response> {
    blocking(move || {
        let session = session(&app.state::<AppState>())?;
        let data = session.read(mft).map_err(err)?;
        let table = formats::sequences::decode(&data).map_err(err)?;
        Ok(Response::new(formats::sequences::to_json(&table).into_bytes()))
    })
    .await
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StarFieldDto {
    header: f32,
    /// Six values per star, see `gw2dat::formats::stars`.
    stars: Vec<f32>,
}

#[tauri::command]
pub async fn load_stars(app: AppHandle, mft: u32) -> CmdResult<StarFieldDto> {
    blocking(move || {
        let session = session(&app.state::<AppState>())?;
        let data = session.read(mft).map_err(err)?;
        let field = formats::stars::decode(&data).map_err(err)?;
        Ok(StarFieldDto { header: field.header, stars: field.stars.into_iter().flatten().collect() })
    })
    .await
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TrueTypeDto {
    family: String,
    subfamily: String,
    full_name: String,
    version: String,
    glyphs: u16,
    characters: Vec<u32>,
}

#[tauri::command]
pub async fn truetype_info(app: AppHandle, mft: u32) -> CmdResult<TrueTypeDto> {
    blocking(move || {
        let session = session(&app.state::<AppState>())?;
        let data = session.read(mft).map_err(err)?;
        let info = formats::truetype::info(&data).map_err(err)?;
        Ok(TrueTypeDto {
            family: info.family,
            subfamily: info.subfamily,
            full_name: info.full_name,
            version: info.version,
            glyphs: info.glyphs,
            characters: info.characters,
        })
    })
    .await
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GlyphFileDto {
    range: usize,
    range_name: &'static str,
    first: u32,
    last: u32,
    fonts: Vec<usize>,
}

#[tauri::command]
pub async fn glyph_file_info(app: AppHandle, mft: u32) -> CmdResult<GlyphFileDto> {
    blocking(move || {
        let session = session(&app.state::<AppState>())?;
        let (range, fonts) = crate::font_export::glyph_owner(&session, mft)?;
        let (first, end, range_name) = formats::font::RANGES[range];
        Ok(GlyphFileDto { range, range_name, first, last: end - 1, fonts })
    })
    .await
}

/// Atlas page of a glyph file; the format string carries `AFNT|<pages>|<glyphs>`.
#[tauri::command]
pub async fn glyph_file_atlas(app: AppHandle, mft: u32, page: usize) -> CmdResult<Response> {
    blocking(move || {
        let session = session(&app.state::<AppState>())?;
        let (mut image, pages, glyphs) = crate::font_export::glyph_file_atlas(&session, mft, page)?;
        image.format = format!("AFNT|{pages}|{glyphs}");
        Ok(image_response(image))
    })
    .await
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FontRangeDto {
    range: usize,
    name: &'static str,
    file_id: u32,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FontDto {
    index: usize,
    extent_x: u8,
    extent_y: u8,
    ranges: Vec<FontRangeDto>,
}

#[tauri::command]
pub async fn load_fonts(app: AppHandle, mft: u32) -> CmdResult<Vec<FontDto>> {
    blocking(move || {
        let session = session(&app.state::<AppState>())?;
        let data = session.read(mft).map_err(err)?;
        let fonts = formats::font::fonts(&data).map_err(err)?;
        Ok(fonts
            .into_iter()
            .map(|f| FontDto {
                index: f.index,
                extent_x: f.extent_x,
                extent_y: f.extent_y,
                ranges: f
                    .files
                    .iter()
                    .enumerate()
                    .filter_map(|(range, id)| {
                        Some(FontRangeDto { range, name: formats::font::RANGES[range].2, file_id: (*id)? })
                    })
                    .collect(),
            })
            .collect())
    })
    .await
}

/// Glyph atlas page as an image response; the page count is appended to the
/// format string as `AFNT|<pages>`.
#[tauri::command]
pub async fn font_atlas(app: AppHandle, mft: u32, font: usize, range: usize, page: usize) -> CmdResult<Response> {
    blocking(move || {
        let session = session(&app.state::<AppState>())?;
        let (mut image, pages) = crate::font_export::atlas(&session, mft, font, range, page)?;
        image.format = format!("AFNT|{pages}");
        Ok(image_response(image))
    })
    .await
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MaterialDto {
    index: u32,
    diffuse: Option<u32>,
    normal: Option<u32>,
    material_file: Option<u32>,
}

#[tauri::command]
pub async fn model_materials(app: AppHandle, mft: u32) -> CmdResult<Vec<MaterialDto>> {
    blocking(move || {
        let session = session(&app.state::<AppState>())?;
        let data = session.read(mft).map_err(err)?;
        let materials = formats::model::materials(&data).map_err(err)?;
        Ok(materials
            .iter()
            .enumerate()
            .map(|(index, m)| MaterialDto {
                index: index as u32,
                diffuse: m.texture(formats::model::TextureSlot::Diffuse),
                normal: m.texture(formats::model::TextureSlot::Normal),
                material_file: m.material_file,
            })
            .collect())
    })
    .await
}

#[tauri::command]
pub fn suggest_file_name(state: State<'_, AppState>, mft: u32, converted: bool) -> CmdResult<String> {
    let session = session(&state)?;
    let entry = session.entry(mft).ok_or("Unknown file")?;
    if !converted && let Some(name) = entry.known_name() {
        return Ok(name.to_string());
    }
    let extension = if converted { formats::converted_extension(entry.file_type) } else { entry.file_type.extension() };
    Ok(format!("{}.{extension}", entry.name()))
}

pub fn path_component(name: &str) -> PathBuf {
    let cleaned: String =
        name.chars().map(|c| if c.is_alphanumeric() || " ._-()x".contains(c) { c } else { '_' }).collect();
    PathBuf::from(cleaned.trim_matches('.').trim())
}
