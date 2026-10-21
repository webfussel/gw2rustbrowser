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

/// The plain TrueType/OpenType (sfnt) data of a font file: TrueType as is,
/// WOFF unpacked, EOT (Embedded OpenType) with its header removed.
pub fn to_sfnt(data: &[u8]) -> Result<std::borrow::Cow<'_, [u8]>> {
    use std::borrow::Cow;
    if data.starts_with(b"wOFF") {
        return woff_to_sfnt(data).map(Cow::Owned);
    }
    if data.len() >= 36 && data[34..36] == [0x4c, 0x50] {
        let u32_le = |at: usize| u32::from_le_bytes(data[at..at + 4].try_into().unwrap()) as usize;
        let (total, font, flags) = (u32_le(0), u32_le(4), u32_le(12));
        if flags & 0x4 != 0 {
            return Err(Error::Unsupported("EOT font with MicroType Express compression".into()));
        }
        let start = total.checked_sub(font).ok_or(Error::Truncated)?;
        let body = data.get(start..total).ok_or(Error::Truncated)?;
        // TTEMBED_XORENCRYPTDATA: every byte XORed with 0x50.
        return Ok(if flags & 0x1000_0000 != 0 {
            Cow::Owned(body.iter().map(|b| b ^ 0x50).collect())
        } else {
            Cow::Borrowed(body)
        });
    }
    Ok(Cow::Borrowed(data))
}

fn woff_to_sfnt(data: &[u8]) -> Result<Vec<u8>> {
    use std::io::Read;
    let r = Reader(data);
    let flavor = r.u32(4)?;
    let tables = r.u16(12)? as usize;
    let mut entries = Vec::with_capacity(tables);
    for i in 0..tables {
        let at = 44 + i * 20;
        let tag = data.get(at..at + 4).ok_or(Error::Truncated)?.to_vec();
        let (offset, compressed, original, checksum) =
            (r.u32(at + 4)? as usize, r.u32(at + 8)? as usize, r.u32(at + 12)? as usize, r.u32(at + 16)?);
        let stored = data.get(offset..offset + compressed).ok_or(Error::Truncated)?;
        let table = if compressed < original {
            let mut out = Vec::with_capacity(original);
            flate2::read::ZlibDecoder::new(stored)
                .read_to_end(&mut out)
                .map_err(|e| Error::format("WOFF", e.to_string()))?;
            out
        } else {
            stored.to_vec()
        };
        entries.push((tag, checksum, table));
    }
    // sfnt offset table, table records, then 4-byte aligned tables.
    let mut power = 1;
    let mut selector = 0u16;
    while power * 2 <= tables {
        power *= 2;
        selector += 1;
    }
    let mut out = Vec::new();
    out.extend_from_slice(&flavor.to_be_bytes());
    out.extend_from_slice(&(tables as u16).to_be_bytes());
    out.extend_from_slice(&((power * 16) as u16).to_be_bytes());
    out.extend_from_slice(&selector.to_be_bytes());
    out.extend_from_slice(&((tables * 16 - power * 16) as u16).to_be_bytes());
    let mut offset = 12 + tables * 16;
    for (tag, checksum, table) in &entries {
        out.extend_from_slice(tag);
        out.extend_from_slice(&checksum.to_be_bytes());
        out.extend_from_slice(&(offset as u32).to_be_bytes());
        out.extend_from_slice(&(table.len() as u32).to_be_bytes());
        offset += table.len().next_multiple_of(4);
    }
    for (_, _, table) in &entries {
        out.extend_from_slice(table);
        out.resize(out.len().next_multiple_of(4), 0);
    }
    Ok(out)
}

/// Names stored in an EOT (Embedded OpenType) header, readable even when the
/// font data itself is compressed with MicroType Express.
pub fn eot_info(data: &[u8]) -> Result<FontInfo> {
    if data.get(34..36) != Some(&[0x4c, 0x50]) {
        return Err(Error::format("EOT font", "missing magic number"));
    }
    let u16_le =
        |at: usize| data.get(at..at + 2).map(|b| u16::from_le_bytes([b[0], b[1]]) as usize).ok_or(Error::Truncated);
    // Padding, size and UTF-16LE string; family, style, version, full name.
    let mut at = 80;
    let mut names = Vec::with_capacity(4);
    for _ in 0..4 {
        let size = u16_le(at + 2)?;
        let bytes = data.get(at + 4..at + 4 + size).ok_or(Error::Truncated)?;
        let units: Vec<u16> = bytes.as_chunks::<2>().0.iter().map(|b| u16::from_le_bytes(*b)).collect();
        names.push(String::from_utf16_lossy(&units).trim_end_matches('\0').to_owned());
        at += 4 + size;
    }
    let [family, subfamily, version, full_name] = names.try_into().unwrap();
    Ok(FontInfo { family, subfamily, full_name, version, ..FontInfo::default() })
}

/// Whether an EOT font's data is compressed with MicroType Express, which is
/// not decoded.
pub fn is_compressed_eot(data: &[u8]) -> bool {
    data.get(34..36) == Some(&[0x4c, 0x50]) && data.get(12..16).is_some_and(|f| f[0] & 0x4 != 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_non_fonts() {
        assert!(info(b"OggS....").is_err());
    }

    #[test]
    fn eot_header_names() {
        let mut data = vec![0u8; 80];
        data[12] = 0x4;
        data[34..36].copy_from_slice(b"LP");
        for name in ["Fam", "Bold", "1.0", "Fam Bold"] {
            let units: Vec<u8> = name.encode_utf16().flat_map(u16::to_le_bytes).collect();
            data.extend_from_slice(&[0, 0]);
            data.extend_from_slice(&(units.len() as u16).to_le_bytes());
            data.extend_from_slice(&units);
        }
        let info = eot_info(&data).unwrap();
        assert_eq!(
            (info.family.as_str(), info.subfamily.as_str(), info.full_name.as_str()),
            ("Fam", "Bold", "Fam Bold")
        );
        assert!(is_compressed_eot(&data));
        assert!(to_sfnt(&data).is_err());
    }
}
