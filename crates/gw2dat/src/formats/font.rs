//! Bitmap fonts (PF `AFNT`): font descriptors whose glyphs live in separate,
//! RLE-compressed files, one per hard-coded character range.

use super::image::RgbaImage;
use super::pf::PackFile;
use crate::error::{Error, Result};
use crate::fourcc::fcc;

pub const AFNT: u32 = fcc(b"AFNT");

/// Character ranges in the order of a font's 13 glyph files (end exclusive).
/// They are compiled into the game client, not stored in the data.
pub const RANGES: [(u32, u32, &str); 13] = [
    (0x00021, 0x0007F, "Basic Latin"),
    (0x000A1, 0x000FF, "Latin-1 Supplement"),
    (0x00100, 0x00180, "Latin Extended-A"),
    (0x00391, 0x00460, "Greek and Cyrillic"),
    (0x02010, 0x0266B, "Symbols"),
    (0x03000, 0x03020, "CJK Symbols and Punctuation"),
    (0x03041, 0x03100, "Hiragana and Katakana"),
    (0x03105, 0x0312A, "Bopomofo"),
    (0x03131, 0x0318F, "Hangul Compatibility Jamo"),
    (0x0AC00, 0x0D7A4, "Hangul Syllables"),
    (0x04E00, 0x09FA6, "CJK Unified Ideographs"),
    (0x0F900, 0x0FA6B, "CJK Compatibility Ideographs"),
    (0x0FF01, 0x0FFE7, "Halfwidth and Fullwidth Forms"),
];

#[derive(Clone, Debug)]
pub struct FontInfo {
    pub index: usize,
    pub extent_x: u8,
    pub extent_y: u8,
    /// Glyph file id per range (see [`RANGES`]).
    pub files: [Option<u32>; 13],
}

/// Reads the font descriptors of an AFNT file.
pub fn fonts(data: &[u8]) -> Result<Vec<FontInfo>> {
    let file = PackFile::parse(data)?;
    let chunk = file.require_chunk(AFNT)?;
    let (count, mut c) = chunk.cursor().array()?;
    let mut fonts = Vec::with_capacity(count.min(256));
    for index in 0..count {
        c.skip(14)?;
        let extent_x = c.u8()?;
        let extent_y = c.u8()?;
        let mut files = [None; 13];
        for slot in &mut files {
            *slot = c.file_ref()?;
        }
        fonts.push(FontInfo { index, extent_x, extent_y, files });
    }
    Ok(fonts)
}

/// All glyph file ids referenced by an AFNT file.
pub fn referenced_files(data: &[u8]) -> Result<Vec<u32>> {
    let mut ids: Vec<u32> = fonts(data)?.iter().flat_map(|f| f.files.iter().flatten().copied()).collect();
    ids.sort_unstable();
    ids.dedup();
    Ok(ids)
}

#[derive(Clone, Debug)]
pub struct Glyph {
    pub character: u32,
    pub y_offset: u8,
    pub width: u32,
    pub height: u32,
    /// Coverage, one byte per pixel.
    pub coverage: Vec<u8>,
}

/// Decodes the glyphs of one range file. A few files end early or contain a
/// glyph that does not decode; the glyphs before it are still returned.
pub fn glyphs(data: &[u8], range: usize) -> Result<Vec<Glyph>> {
    let (glyphs, error) = glyphs_partial(data, range)?;
    match error {
        Some(error) if glyphs.is_empty() => Err(error),
        _ => Ok(glyphs),
    }
}

fn glyphs_partial(data: &[u8], range: usize) -> Result<(Vec<Glyph>, Option<Error>)> {
    let (start, end, _) = *RANGES.get(range).ok_or_else(|| Error::format("bitmap font", "no such range"))?;
    let mut pos = 0usize;
    let mut glyphs = Vec::with_capacity((end - start) as usize);
    for character in start..end {
        match decode_glyph(data, &mut pos, character) {
            Ok(glyph) => glyphs.push(glyph),
            Err(error) => return Ok((glyphs, Some(error))),
        }
    }
    Ok((glyphs, None))
}

fn decode_glyph(data: &[u8], pos: &mut usize, character: u32) -> Result<Glyph> {
    let byte = |pos: usize| data.get(pos).copied().ok_or(Error::Truncated);
    {
        let y_offset = byte(*pos)?;
        let width = byte(*pos + 1)? as u32 + 1;
        let height = byte(*pos + 2)? as u32 + 1;
        let rle = byte(*pos + 3)?;
        *pos += 4;
        let mut remaining = (width * height) as usize;
        let mut coverage = Vec::with_capacity(remaining);
        // Types 0 and 255 alternate between empty and full runs (starting
        // with the opposite of 255 - type); type 1 stores the value per run.
        let alternating = rle == 0 || rle == 255;
        let mut value: u8 = if alternating { 255 - rle } else { 0 };
        while remaining > 0 {
            let mut run = 0usize;
            if alternating {
                value = 255 - value;
            } else {
                value = byte(*pos)?;
                *pos += 1;
                if value != 0 && value != 255 {
                    run = 1;
                }
            }
            if run == 0 {
                run = 1;
                loop {
                    let b = byte(*pos)?;
                    *pos += 1;
                    run += b as usize;
                    if b != 255 {
                        break;
                    }
                }
            }
            if run > remaining {
                return Err(Error::format("bitmap font", "glyph run exceeds its size"));
            }
            coverage.extend(std::iter::repeat_n(value, run));
            remaining -= run;
        }
        Ok(Glyph { character, y_offset, width, height, coverage })
    }
}

/// Lays out glyphs in a grid (white glyphs on transparent background).
pub fn atlas(glyphs: &[Glyph], columns: usize) -> RgbaImage {
    let cell_w = glyphs.iter().map(|g| g.width).max().unwrap_or(1) + 2;
    let cell_h = glyphs.iter().map(|g| g.height + g.y_offset as u32).max().unwrap_or(1) + 2;
    let columns = columns.max(1).min(glyphs.len().max(1));
    let rows = glyphs.len().div_ceil(columns).max(1);
    let (width, height) = (cell_w * columns as u32, cell_h * rows as u32);
    let mut pixels = vec![0u8; (width * height * 4) as usize];
    for (i, g) in glyphs.iter().enumerate() {
        let (ox, oy) = ((i % columns) as u32 * cell_w + 1, (i / columns) as u32 * cell_h + 1 + g.y_offset as u32);
        for y in 0..g.height {
            for x in 0..g.width {
                let a = g.coverage[(y * g.width + x) as usize];
                let at = (((oy + y) * width + ox + x) * 4) as usize;
                pixels[at..at + 4].copy_from_slice(&[255, 255, 255, a]);
            }
        }
    }
    RgbaImage { width, height, pixels, format: "AFNT".into() }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rle_glyph() {
        // 2x2 glyph, type 0: runs alternate starting with empty.
        // run 1 empty (1 + 0), run 3 full (1 + 2).
        let glyph = [5, 1, 1, 0, 0, 2];
        let mut pos = 0;
        let g = decode_glyph(&glyph, &mut pos, 'A' as u32).unwrap();
        assert_eq!((g.width, g.height, g.y_offset), (2, 2, 5));
        assert_eq!(g.coverage, [0, 255, 255, 255]);
        assert_eq!(pos, glyph.len());
    }

    #[test]
    fn rle_glyph_with_literal_values() {
        // Type 1: value per run; values other than 0/255 are single pixels.
        let glyph = [0, 2, 0, 1, 128, 0, 1];
        let mut pos = 0;
        let g = decode_glyph(&glyph, &mut pos, 'B' as u32).unwrap();
        assert_eq!(g.coverage, [128, 0, 0]);
    }
}
