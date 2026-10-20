//! Bitmap fonts: glyph atlases for preview and export.

use std::path::Path;

use gw2dat::formats::{font, image::RgbaImage};

use crate::session::Session;

/// Glyphs per atlas page; the CJK ranges have tens of thousands.
pub const PAGE_SIZE: usize = 1024;

pub fn atlas(
    session: &Session,
    mft: u32,
    font_index: usize,
    range: usize,
    page: usize,
) -> Result<(RgbaImage, usize), String> {
    let data = session.read(mft).map_err(|e| e.to_string())?;
    let fonts = font::fonts(&data).map_err(|e| e.to_string())?;
    let info = fonts.get(font_index).ok_or("No such font")?;
    let file_id = info.files.get(range).copied().flatten().ok_or("This font has no glyphs in that range")?;
    let mft = session.archive.entry_for_id(file_id).ok_or_else(|| format!("Glyph file {file_id} is missing"))?;
    let glyphs =
        font::glyphs(&session.archive.read(mft).map_err(|e| e.to_string())?, range).map_err(|e| e.to_string())?;
    let pages = glyphs.len().div_ceil(PAGE_SIZE).max(1);
    let start = (page.min(pages - 1)) * PAGE_SIZE;
    let slice = &glyphs[start..(start + PAGE_SIZE).min(glyphs.len())];
    let columns = (slice.len() as f64).sqrt().ceil().max(16.0) as usize;
    Ok((font::atlas(slice, columns), pages))
}

/// Writes one PNG atlas per font and range into `dir`.
pub fn write_fonts(session: &Session, mft: u32, dir: &Path) -> Result<(), String> {
    let data = session.archive.read(mft).map_err(|e| e.to_string())?;
    let fonts = font::fonts(&data).map_err(|e| e.to_string())?;
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    for info in &fonts {
        for (range, file_id) in info.files.iter().enumerate() {
            let Some(file_id) = file_id else { continue };
            let Some(glyph_mft) = session.archive.entry_for_id(*file_id) else { continue };
            let Ok(glyph_data) = session.archive.read(glyph_mft) else { continue };
            let Ok(glyphs) = font::glyphs(&glyph_data, range) else { continue };
            let columns = (glyphs.len() as f64).sqrt().ceil().max(16.0) as usize;
            let png = font::atlas(&glyphs, columns).to_png().map_err(|e| e.to_string())?;
            let name = format!(
                "font{:03}_{}x{}_{:02}_{}.png",
                info.index,
                info.extent_x,
                info.extent_y,
                range,
                font::RANGES[range].2.replace(' ', "_")
            );
            std::fs::write(dir.join(name), png).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}
