//! Game texts through the TextPack files: text lookup by id, variants, voice
//! lines and the index from voice ids to sound bank entries.

use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex, OnceLock};

use gw2dat::FileType;
use gw2dat::formats::{audio, text, textpack};
use rayon::prelude::*;
use serde::Serialize;

use crate::session::Session;

/// Decoded strings files kept in memory (each holds `strings_per_file` texts).
const STRINGS_CACHE: usize = 1024;

pub struct Texts {
    pub manifest: textpack::Manifest,
    pub variants: Vec<(u32, Vec<u32>)>,
    pub voices: Vec<(u32, u32)>,
    voice_of_text: HashMap<u32, u32>,
    strings: Mutex<HashMap<u32, Arc<Vec<Option<String>>>>>,
}

/// Voice id -> (sound bank MFT index, entry index, playable).
pub type VoiceIndex = HashMap<u32, (u32, usize, bool)>;

#[derive(Default)]
pub struct TextState {
    texts: OnceLock<Result<Arc<Texts>, String>>,
    voice_index: OnceLock<Arc<VoiceIndex>>,
    building: Mutex<()>,
}

fn find(session: &Session, file_type: FileType) -> Option<Vec<u8>> {
    let entry = session.index.entries.iter().find(|e| e.file_type == file_type)?;
    session.archive.read(entry.mft_index).ok()
}

impl TextState {
    pub fn texts(&self, session: &Session) -> Result<Arc<Texts>, String> {
        self.texts
            .get_or_init(|| {
                let manifest = find(session, FileType::TextPackManifest).ok_or("The archive has no text manifest")?;
                let manifest = textpack::manifest(&manifest).map_err(|e| e.to_string())?;
                let variants = find(session, FileType::TextPackVariant)
                    .and_then(|d| textpack::variants(&d).ok())
                    .unwrap_or_default();
                let voices =
                    find(session, FileType::TextPackVoices).and_then(|d| textpack::voices(&d).ok()).unwrap_or_default();
                let voice_of_text = voices.iter().copied().collect();
                Ok(Arc::new(Texts { manifest, variants, voices, voice_of_text, strings: Mutex::default() }))
            })
            .clone()
    }

    pub fn voice_index(&self) -> Option<Arc<VoiceIndex>> {
        self.voice_index.get().cloned()
    }

    /// Builds (or loads from `cache`) the voice index; slow the first time.
    pub fn ensure_voice_index(&self, session: &Session, cache: Option<&Path>) -> Arc<VoiceIndex> {
        let _guard = self.building.lock().unwrap();
        if let Some(index) = self.voice_index.get() {
            return index.clone();
        }
        let banks: Vec<_> = session.index.entries.iter().filter(|e| e.file_type == FileType::Bank).collect();
        // Fingerprint of all bank entries: changes whenever a patch touches one.
        let mut fingerprint: u64 = 0xcbf2_9ce4_8422_2325;
        for b in &banks {
            for v in [b.mft_index as u64, b.stored_offset, b.stored_size as u64] {
                fingerprint = (fingerprint ^ v).wrapping_mul(0x0100_0000_01b3);
            }
        }
        let index = cache.and_then(|path| load_voice_index(path, fingerprint)).unwrap_or_else(|| {
            let index: VoiceIndex = banks
                .par_iter()
                .flat_map_iter(|b| {
                    let voices = session.archive.read(b.mft_index).ok().and_then(|d| audio::bank_voices(&d).ok());
                    let mft = b.mft_index;
                    voices.unwrap_or_default().into_iter().map(move |(voice, i, playable)| (voice, (mft, i, playable)))
                })
                .collect();
            if let Some(path) = cache {
                save_voice_index(path, fingerprint, &index);
            }
            index
        });
        let index = Arc::new(index);
        let _ = self.voice_index.set(index.clone());
        index
    }
}

fn save_voice_index(path: &Path, fingerprint: u64, index: &VoiceIndex) {
    let mut out = Vec::with_capacity(16 + index.len() * 13);
    out.extend_from_slice(b"GW2VOICE");
    out.extend_from_slice(&fingerprint.to_le_bytes());
    for (&voice, &(mft, entry, playable)) in index {
        out.extend_from_slice(&voice.to_le_bytes());
        out.extend_from_slice(&mft.to_le_bytes());
        out.extend_from_slice(&(entry as u32).to_le_bytes());
        out.push(playable as u8);
    }
    let _ = std::fs::write(path, out);
}

fn load_voice_index(path: &Path, fingerprint: u64) -> Option<VoiceIndex> {
    let data = std::fs::read(path).ok()?;
    if data.get(..8)? != b"GW2VOICE" || u64::from_le_bytes(data.get(8..16)?.try_into().ok()?) != fingerprint {
        return None;
    }
    let u32_at = |b: &[u8], at: usize| u32::from_le_bytes(b[at..at + 4].try_into().unwrap());
    Some(
        data[16..]
            .as_chunks::<13>()
            .0
            .iter()
            .map(|r| (u32_at(r, 0), (u32_at(r, 4), u32_at(r, 8) as usize, r[12] != 0)))
            .collect(),
    )
}

impl Texts {
    /// Text of an id: `Ok(Some)` plain, `Ok(None)` encrypted, `Err` missing.
    pub fn lookup(&self, session: &Session, language: usize, id: u32) -> Result<Option<String>, ()> {
        let (file, entry) = self.manifest.locate(language, id).ok_or(())?;
        let strings = self.strings_file(session, file).ok_or(())?;
        strings.get(entry).cloned().ok_or(())
    }

    fn strings_file(&self, session: &Session, file: u32) -> Option<Arc<Vec<Option<String>>>> {
        if let Some(cached) = self.strings.lock().unwrap().get(&file) {
            return Some(cached.clone());
        }
        let data = session.archive.read(session.archive.entry_for_id(file)?).ok()?;
        let decoded = text::decode_strings(&data).ok()?;
        let texts = Arc::new(decoded.entries.into_iter().map(|e| e.text).collect::<Vec<_>>());
        let mut cache = self.strings.lock().unwrap();
        if cache.len() >= STRINGS_CACHE {
            cache.clear();
        }
        cache.insert(file, texts.clone());
        Some(texts)
    }

    pub fn voice_of(&self, text: u32) -> Option<u32> {
        self.voice_of_text.get(&text).copied()
    }
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct TextRow {
    id: u32,
    /// The text; `None` when encrypted or missing (see `state`).
    text: Option<String>,
    /// "plain", "encrypted" or "missing".
    state: &'static str,
    voice: Option<u32>,
    /// Whether the voice line can be played; `None` while the voice index is
    /// not built yet.
    playable: Option<bool>,
}

pub fn row(session: &Session, texts: &Texts, voices: Option<&VoiceIndex>, language: usize, id: u32) -> TextRow {
    let (text, state) = match texts.lookup(session, language, id) {
        Ok(Some(t)) => (Some(t), "plain"),
        Ok(None) => (None, "encrypted"),
        Err(()) => (None, "missing"),
    };
    let voice = texts.voice_of(id);
    let playable = voice.and_then(|v| voices.map(|index| index.get(&v).is_some_and(|e| e.2)));
    TextRow { id, text, state, voice, playable }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LanguageDto {
    index: usize,
    name: &'static str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TextPackInfo {
    languages: Vec<LanguageDto>,
    text_count: u32,
    strings_per_file: u32,
    variant_count: usize,
    voice_count: usize,
    voice_index_ready: bool,
}

pub fn info(state: &TextState, session: &Session) -> Result<TextPackInfo, String> {
    let texts = state.texts(session)?;
    Ok(TextPackInfo {
        languages: texts
            .manifest
            .languages
            .iter()
            .enumerate()
            .filter(|(_, files)| !files.is_empty())
            .map(|(index, _)| LanguageDto { index, name: text::language_name(index as u8) })
            .collect(),
        text_count: texts.manifest.text_count(),
        strings_per_file: texts.manifest.strings_per_file,
        variant_count: texts.variants.len(),
        voice_count: texts.voices.len(),
        voice_index_ready: state.voice_index().is_some(),
    })
}

/// Text ids containing `query` (case-insensitive), searched in parallel over
/// all strings files of the language.
pub fn search(session: &Session, texts: &Texts, language: usize, query: &str, limit: usize) -> Vec<u32> {
    let needle = query.to_lowercase();
    let per_file = texts.manifest.strings_per_file;
    let files = texts.manifest.languages.get(language).cloned().unwrap_or_default();
    let mut hits: Vec<u32> = files
        .par_iter()
        .enumerate()
        .flat_map_iter(|(file_index, file)| {
            let strings = file.and_then(|f| texts.strings_file(session, f));
            let needle = needle.clone();
            strings.into_iter().flat_map(move |s| {
                let needle = needle.clone();
                s.iter()
                    .enumerate()
                    .filter(move |(_, t)| t.as_ref().is_some_and(|t| t.to_lowercase().contains(&needle)))
                    .map(move |(i, _)| file_index as u32 * per_file + i as u32)
                    .collect::<Vec<_>>()
            })
        })
        .collect();
    hits.sort_unstable();
    hits.truncate(limit);
    hits
}
