//! Writing models as OBJ + MTL with their textures as PNG.

use std::path::Path;

use gw2dat::formats::{image, model};

use crate::session::Session;

/// Writes `<dir>/<name>.obj`, `<dir>/<name>.mtl` and the diffuse and normal
/// textures as `<dir>/<texture id>.png`. Textures that fail to decode are
/// skipped; the model is still written.
pub fn write_model(session: &Session, mft: u32, dir: &Path, name: &str) -> Result<(), String> {
    let data = session.archive.read(mft).map_err(|e| e.to_string())?;
    let geometry = model::decode(&data).map_err(|e| e.to_string())?;
    let materials = model::materials(&data).unwrap_or_default();
    let (obj, mtl) = model::to_obj_mtl(&geometry, &materials, name, |id| format!("{id}.png"));
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    std::fs::write(dir.join(format!("{name}.obj")), obj).map_err(|e| e.to_string())?;
    std::fs::write(dir.join(format!("{name}.mtl")), mtl).map_err(|e| e.to_string())?;

    let mut textures: Vec<u32> = geometry
        .meshes
        .iter()
        .filter_map(|m| materials.get(m.material_index as usize))
        .flat_map(|m| [m.texture(model::TextureSlot::Diffuse), m.texture(model::TextureSlot::Normal)])
        .flatten()
        .collect();
    textures.sort_unstable();
    textures.dedup();
    for id in textures {
        let target = dir.join(format!("{id}.png"));
        if target.exists() {
            continue;
        }
        let png = session
            .archive
            .entry_for_id(id)
            .and_then(|mft| session.archive.read(mft).ok())
            .and_then(|data| image::decode(&data).ok())
            .and_then(|img| img.to_png().ok());
        if let Some(png) = png {
            let _ = std::fs::write(target, png);
        }
    }
    Ok(())
}
