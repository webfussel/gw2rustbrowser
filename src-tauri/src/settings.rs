//! Small persisted settings (last opened archive) and the index cache location.

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use tauri::{AppHandle, Manager};

/// Folder for settings, caches and the web view's data: `data` next to the
/// executable, so an unpacked copy of the app keeps everything in its own
/// folder. `None` when that folder cannot be written (e.g. a read-only
/// install location); the per-user folders are used then.
pub fn portable_dir() -> Option<&'static Path> {
    static DIR: OnceLock<Option<PathBuf>> = OnceLock::new();
    DIR.get_or_init(|| {
        let dir = std::env::current_exe().ok()?.parent()?.join("data");
        std::fs::create_dir_all(&dir).ok()?;
        let probe = dir.join(".write-test");
        std::fs::write(&probe, b"").ok()?;
        let _ = std::fs::remove_file(&probe);
        Some(dir)
    })
    .as_deref()
}

fn settings_file(app: &AppHandle) -> Option<PathBuf> {
    let dir = match portable_dir() {
        Some(dir) => dir.to_path_buf(),
        None => app.path().app_config_dir().ok()?,
    };
    Some(dir.join("settings.json"))
}

pub fn last_dat(app: &AppHandle) -> Option<String> {
    let text = std::fs::read_to_string(settings_file(app)?).ok()?;
    let value: serde_json::Value = serde_json::from_str(&text).ok()?;
    value.get("lastDat")?.as_str().map(str::to_string)
}

pub fn set_last_dat(app: &AppHandle, path: &Path) {
    let Some(file) = settings_file(app) else { return };
    if let Some(dir) = file.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let value = serde_json::json!({ "lastDat": path.to_string_lossy() });
    let _ = std::fs::write(file, value.to_string());
}

/// Cache file for the voice index of an archive.
pub fn voice_cache(app: &AppHandle, dat: &Path) -> Option<PathBuf> {
    Some(index_cache(app, dat)?.with_extension("voices"))
}

/// One cache file per archive path (FNV-1a hash, stable across builds).
pub fn index_cache(app: &AppHandle, dat: &Path) -> Option<PathBuf> {
    let dir = match portable_dir() {
        Some(dir) => dir.join("cache"),
        None => app.path().app_cache_dir().ok()?,
    };
    std::fs::create_dir_all(&dir).ok()?;
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in dat.to_string_lossy().bytes() {
        hash = (hash ^ byte as u64).wrapping_mul(0x0100_0000_01b3);
    }
    Some(dir.join(format!("index-{hash:016x}.bin")))
}
