mod commands;
mod export;
mod font_export;
mod model_export;
mod session;
mod settings;

use std::sync::atomic::AtomicBool;
use std::sync::{Arc, RwLock};

use gw2dat::ScanProgress;

#[derive(Default)]
pub struct AppState {
    pub session: RwLock<Option<Arc<session::Session>>>,
    pub scan: Arc<ScanProgress>,
    pub cancel_export: Arc<AtomicBool>,
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState::default())
        .invoke_handler(tauri::generate_handler![
            commands::startup_path,
            commands::open_dat,
            commands::categories,
            commands::category_files,
            commands::entry_info,
            commands::find,
            commands::read_file,
            commands::load_image,
            commands::load_sound,
            commands::load_bank,
            commands::load_bank_sound,
            commands::load_strings,
            commands::load_eula,
            commands::load_model,
            commands::model_materials,
            commands::load_texture,
            commands::load_fonts,
            commands::load_text,
            commands::load_animation,
            commands::model_summary,
            commands::font_atlas,
            commands::truetype_info,
            commands::glyph_file_info,
            commands::glyph_file_atlas,
            commands::suggest_file_name,
            export::export_file,
            export::export_files,
            export::export_bank_sound,
            export::cancel_export,
        ])
        .run(tauri::generate_context!())
        .expect("error while running the application");
}
