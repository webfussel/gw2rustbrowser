//! Exporting single files and whole categories.

use std::path::PathBuf;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU32, Ordering};

use gw2dat::formats;
use rayon::prelude::*;
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};

use crate::AppState;
use crate::commands::{CmdResult, blocking, path_component, session};

fn export_data(
    session: &crate::session::Session,
    mft: u32,
    converted: bool,
) -> gw2dat::Result<(Vec<u8>, &'static str)> {
    let entry = session.entry(mft).ok_or(gw2dat::Error::NoSuchEntry(mft))?;
    let data = session.archive.read(mft)?;
    if converted {
        let c = formats::convert(entry.file_type, data)?;
        Ok((c.data, c.extension))
    } else {
        Ok((data, entry.file_type.extension()))
    }
}

/// Output files for one entry: `(file name suffix, data, extension)`.
fn export_outputs(
    session: &crate::session::Session,
    mft: u32,
    converted: bool,
) -> gw2dat::Result<Vec<(String, Vec<u8>, &'static str)>> {
    let entry = session.entry(mft).ok_or(gw2dat::Error::NoSuchEntry(mft))?;
    let data = session.archive.read(mft)?;
    if !converted {
        return Ok(vec![(String::new(), data, entry.file_type.extension())]);
    }
    Ok(formats::convert_all(entry.file_type, data)?
        .into_iter()
        .map(|(suffix, c)| (suffix.unwrap_or_default(), c.data, c.extension))
        .collect())
}

#[tauri::command]
pub async fn export_file(app: AppHandle, mft: u32, path: String, converted: bool) -> CmdResult<()> {
    blocking(move || {
        let session = session(&app.state::<AppState>())?;
        let is_model = session.entry(mft).is_some_and(|e| e.file_type.kind() == gw2dat::Kind::Model);
        if converted && is_model {
            // OBJ + MTL + textures next to the chosen file.
            let path = std::path::Path::new(&path);
            let dir = path.parent().ok_or("Invalid path")?;
            let name = path.file_stem().ok_or("Invalid path")?.to_string_lossy();
            return crate::model_export::write_model(&session, mft, dir, &name);
        }
        if converted && session.entry(mft).is_some_and(|e| e.file_type.kind() == gw2dat::Kind::FontGlyphs) {
            return crate::font_export::write_glyph_file(&session, mft, std::path::Path::new(&path));
        }
        let is_font = session.entry(mft).is_some_and(|e| e.file_type.kind() == gw2dat::Kind::BitmapFont);
        if converted && is_font {
            // A folder of glyph atlases; the chosen path names the folder.
            let path = std::path::Path::new(&path);
            return crate::font_export::write_fonts(&session, mft, &path.with_extension(""));
        }
        let (data, _) = export_data(&session, mft, converted).map_err(|e| e.to_string())?;
        std::fs::write(&path, data).map_err(|e| format!("Writing {path} failed: {e}"))
    })
    .await
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ExportProgress {
    done: u32,
    total: u32,
    failed: u32,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportSummary {
    written: u32,
    failed: u32,
    cancelled: bool,
    errors: Vec<String>,
    directory: String,
}

/// Exports every file of a category (recursively) or an explicit list of
/// files into `directory`, keeping the category structure.
#[tauri::command]
pub async fn export_files(
    app: AppHandle,
    category: Option<u32>,
    files: Option<Vec<u32>>,
    directory: String,
    converted: bool,
) -> CmdResult<ExportSummary> {
    blocking(move || {
        let state = app.state::<AppState>();
        let session = session(&state)?;
        state.cancel_export.store(false, Ordering::Relaxed);

        let mut root = PathBuf::from(&directory);
        let entries: Vec<u32> = match (category, files) {
            (Some(category), _) => {
                let name = session.index.categories.get(category as usize).ok_or("Unknown category")?.name.clone();
                root.push(path_component(&name));
                session
                    .files_recursive(category)
                    .into_iter()
                    .map(|i| session.index.entries[i as usize].mft_index)
                    .collect()
            }
            (None, Some(files)) => files,
            (None, None) => return Err("Nothing to export".into()),
        };
        // Category path below the exported category, per entry.
        let base_depth = category.map_or(usize::MAX, |c| session.index.category_path(c).len());

        let total = entries.len() as u32;
        let (done, failed) = (AtomicU32::new(0), AtomicU32::new(0));
        let errors = Mutex::new(Vec::new());
        let emit = |done: u32| {
            let _ = app.emit("export-progress", ExportProgress { done, total, failed: failed.load(Ordering::Relaxed) });
        };
        emit(0);

        entries.par_iter().for_each(|&mft| {
            if state.cancel_export.load(Ordering::Relaxed) {
                return;
            }
            let result = (|| -> Result<(), String> {
                let entry = session.entry(mft).ok_or("unknown file")?;
                let mut dir = root.clone();
                if base_depth != usize::MAX {
                    for part in session.index.category_path(entry.category).iter().skip(base_depth) {
                        dir.push(path_component(part));
                    }
                }
                if converted && entry.file_type.kind() == gw2dat::Kind::Model {
                    // Each model gets its own folder for OBJ, MTL and textures.
                    return crate::model_export::write_model(&session, mft, &dir.join(entry.name()), &entry.name());
                }
                if converted && entry.file_type.kind() == gw2dat::Kind::BitmapFont {
                    return crate::font_export::write_fonts(&session, mft, &dir.join(entry.name()));
                }
                if converted && entry.file_type.kind() == gw2dat::Kind::FontGlyphs {
                    return crate::font_export::write_glyph_file(
                        &session,
                        mft,
                        &dir.join(format!("{}.png", entry.name())),
                    );
                }
                let outputs = export_outputs(&session, mft, converted).map_err(|e| e.to_string())?;
                std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
                for (suffix, data, extension) in outputs {
                    std::fs::write(dir.join(format!("{}{suffix}.{extension}", entry.name())), data)
                        .map_err(|e| e.to_string())?;
                }
                Ok(())
            })();
            if let Err(e) = result {
                failed.fetch_add(1, Ordering::Relaxed);
                let mut errors = errors.lock().unwrap();
                if errors.len() < 20 {
                    errors.push(format!("{}: {e}", session.entry(mft).map_or(mft.to_string(), |e| e.name())));
                }
            }
            let n = done.fetch_add(1, Ordering::Relaxed) + 1;
            if n % 64 == 0 {
                emit(n);
            }
        });
        let finished = done.load(Ordering::Relaxed);
        emit(finished);
        let failed = failed.load(Ordering::Relaxed);
        Ok(ExportSummary {
            written: finished - failed,
            failed,
            cancelled: state.cancel_export.load(Ordering::Relaxed),
            errors: errors.into_inner().unwrap(),
            directory: root.to_string_lossy().into_owned(),
        })
    })
    .await
}

/// Exports one entry of a sound bank.
#[tauri::command]
pub async fn export_bank_sound(app: AppHandle, mft: u32, index: usize, path: String) -> CmdResult<()> {
    blocking(move || {
        let session = session(&app.state::<AppState>())?;
        let data = session.read(mft).map_err(|e| e.to_string())?;
        let entries = formats::audio::decode_bank(&data).map_err(|e| e.to_string())?;
        let entry = entries.into_iter().find(|e| e.index == index).ok_or("No such bank entry")?;
        let sound = entry.sound.map_err(|e| e.to_string())?;
        std::fs::write(&path, sound.data).map_err(|e| format!("Writing {path} failed: {e}"))
    })
    .await
}

#[tauri::command]
pub fn cancel_export(state: State<'_, AppState>) {
    state.cancel_export.store(true, Ordering::Relaxed);
}
