//! Small persisted settings (last opened archive) and the index cache location.

use std::path::{Path, PathBuf};

use tauri::{AppHandle, Manager};

fn settings_file(app: &AppHandle) -> Option<PathBuf> {
    Some(app.path().app_config_dir().ok()?.join("settings.json"))
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

/// One cache file per archive path (FNV-1a hash, stable across builds).
pub fn index_cache(app: &AppHandle, dat: &Path) -> Option<PathBuf> {
    let dir = app.path().app_cache_dir().ok()?;
    std::fs::create_dir_all(&dir).ok()?;
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in dat.to_string_lossy().bytes() {
        hash = (hash ^ byte as u64).wrapping_mul(0x0100_0000_01b3);
    }
    Some(dir.join(format!("index-{hash:016x}.bin")))
}
