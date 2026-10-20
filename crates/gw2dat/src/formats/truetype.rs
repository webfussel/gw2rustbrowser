//! Metadata of TrueType fonts: names, glyph count and the characters they map.

use crate::error::{Error, Result};

#[derive(Clone, Debug, Default)]
pub struct FontInfo {
    pub family: String,
    pub subfamily: String,
    pub full_name: String,
    pub version: String,
    pub glyphs: u16,
    /// Unicode code points with a glyph, ascending.
    pub characters: Vec<u32>,
}

struct Reader<'a>(&'a [u8]);

impl Reader<'_> {
    fn u16(&self, at: usize) -> Result<u16> {
        self.0.get(at..at + 2).map(|b| u16::from_be_bytes([b[0], b[1]])).ok_or(Error::Truncated)
    }
    fn u32(&self, at: usize) -> Result<u32> {
        self.0.get(at..at + 4).map(|b| u32::from_be_bytes(b.try_into().unwrap())).ok_or(Error::Truncated)
    }
}

fn find_table<'a>(data: &'a [u8], tag: &[u8; 4]) -> Result<Option<&'a [u8]>> {
    let r = Reader(data);
    for i in 0..r.u16(4)? as usize {
        let record = 12 + i * 16;
        if data.get(record..record + 4) == Some(tag) {
            let (offset, length) = (r.u32(record + 8)? as usize, r.u32(record + 12)? as usize);
            return Ok(Some(data.get(offset..offset + length).ok_or(Error::Truncated)?));
        }
    }
    Ok(None)
}

pub fn is_truetype(data: &[u8]) -> bool {
    data.starts_with(&[0, 1, 0, 0]) || data.starts_with(b"true")
}

pub fn info(data: &[u8]) -> Result<FontInfo> {
    if !is_truetype(data) {
        return Err(Error::format("TrueType font", "missing sfnt signature"));
    }
    let mut info = FontInfo::default();
    if let Some(maxp) = find_table(data, b"maxp")? {
        info.glyphs = Reader(maxp).u16(4)?;
    }
    if let Some(name) = find_table(data, b"name")? {
        read_names(name, &mut info)?;
    }
    if let Some(cmap) = find_table(data, b"cmap")? {
        info.characters = read_cmap(cmap)?;
    }
    Ok(info)
}

fn read_names(table: &[u8], info: &mut FontInfo) -> Result<()> {
    let r = Reader(table);
    let (count, storage) = (r.u16(2)? as usize, r.u16(4)? as usize);
    // Best record per name id: Windows Unicode (US English first) over Mac Roman.
    let mut best: [(u8, String); 6] = Default::default();
    for i in 0..count {
        let at = 6 + i * 12;
        let (platform, encoding, language, id) = (r.u16(at)?, r.u16(at + 2)?, r.u16(at + 4)?, r.u16(at + 6)? as usize);
        let (length, offset) = (r.u16(at + 8)? as usize, r.u16(at + 10)? as usize);
        if !matches!(id, 1 | 2 | 4 | 5) {
            continue;
        }
        let Some(bytes) = table.get(storage + offset..storage + offset + length) else { continue };
        let (rank, text) = match (platform, encoding) {
            (3, 1 | 10) | (0, _) => {
                let units: Vec<u16> = bytes.as_chunks::<2>().0.iter().map(|&b| u16::from_be_bytes(b)).collect();
                (if platform == 3 && language == 0x409 { 3 } else { 2 }, String::from_utf16_lossy(&units))
            }
            (1, 0) => (1, bytes.iter().map(|&b| b as char).collect()),
            _ => continue,
        };
        if rank > best[id].0 {
            best[id] = (rank, text);
        }
    }
    // Some webfonts scramble their names with control characters.
    let clean = |text: String| text.chars().filter(|c| !c.is_control()).collect::<String>().trim().to_string();
    let [_, family, subfamily, _, full_name, version] = best.map(|(_, text)| clean(text));
    *info = FontInfo { family, subfamily, full_name, version, ..std::mem::take(info) };
    Ok(())
}

fn read_cmap(table: &[u8]) -> Result<Vec<u32>> {
    let r = Reader(table);
    let mut subtable = None;
    for i in 0..r.u16(2)? as usize {
        let at = 4 + i * 8;
        let (platform, encoding, offset) = (r.u16(at)?, r.u16(at + 2)?, r.u32(at + 4)? as usize);
        let format = r.u16(offset)?;
        // Prefer full Unicode (format 12) over the BMP-only format 4.
        let rank = match (platform, encoding, format) {
            (3, 10, 12) | (0, 4 | 6, 12) => 3,
            (3, 1, 4) | (0, _, 4) => 2,
            _ => 0,
        };
        if rank > 0 && subtable.is_none_or(|(best, _)| rank > best) {
            subtable = Some((rank, offset));
        }
    }
    let Some((_, offset)) = subtable else { return Ok(Vec::new()) };
    let mut characters = Vec::new();
    match r.u16(offset)? {
        4 => {
            let segments = r.u16(offset + 6)? as usize / 2;
            let ends = offset + 14;
            let starts = ends + segments * 2 + 2;
            let deltas = starts + segments * 2;
            let range_offsets = deltas + segments * 2;
            for s in 0..segments {
                let (start, end) = (r.u16(starts + s * 2)? as u32, r.u16(ends + s * 2)? as u32);
                let delta = r.u16(deltas + s * 2)? as u32;
                let range_offset = r.u16(range_offsets + s * 2)? as usize;
                for c in start..=end.min(0xfffe) {
                    let glyph = if range_offset == 0 {
                        (c + delta) & 0xffff
                    } else {
                        let at = range_offsets + s * 2 + range_offset + (c - start) as usize * 2;
                        match r.u16(at)? as u32 {
                            0 => 0,
                            g => (g + delta) & 0xffff,
                        }
                    };
                    if glyph != 0 {
                        characters.push(c);
                    }
                }
            }
        }
        12 => {
            for g in 0..r.u32(offset + 12)? as usize {
                let at = offset + 16 + g * 12;
                let (start, end) = (r.u32(at)?, r.u32(at + 4)?);
                characters.extend(start..=end.min(start.saturating_add(0xffff)));
            }
        }
        _ => {}
    }
    characters.sort_unstable();
    characters.dedup();
    Ok(characters)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_non_fonts() {
        assert!(info(b"OggS....").is_err());
    }
}
