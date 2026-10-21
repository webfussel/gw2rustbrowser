mod audio_script_view;
mod commands;
mod composite_view;
mod emote_view;
mod export;
mod font_export;
mod manifest_view;
mod map_extras_view;
mod map_view;
mod material_view;
mod model_export;
mod paged_image_view;
mod rig;
mod session;
mod settings;
mod texts;

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
            commands::load_bank_index,
            audio_script_view::audio_script_info,
            composite_view::composite_info,
            composite_view::composite_race,
            emote_view::emote_info,
            manifest_view::collision_manifest_info,
            map_extras_view::collision_info,
            map_extras_view::shadow_info,
            map_extras_view::shadow_image,
            map_extras_view::map_metadata,
            manifest_view::portal_manifest_info,
            manifest_view::portal_start_models,
            paged_image_view::paged_image_info,
            paged_image_view::paged_image_overview,
            paged_image_view::paged_image_tile,
            paged_image_view::export_paged_image,
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
            commands::model_rig,
            commands::model_clip,
            commands::font_atlas,
            commands::truetype_info,
            commands::load_stars,
            map_view::load_map,
            map_view::load_prop_model,
            material_view::material_info,
            material_view::material_shader,
            commands::load_sequences,
            commands::textpack_info,
            commands::text_rows,
            commands::variant_rows,
            commands::voice_rows,
            commands::search_texts,
            commands::build_voice_index,
            commands::play_voice,
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
