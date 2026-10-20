//! Text-like formats: strings tables and EULA files.

use super::pf::{self, PackFile};
use crate::error::{Error, Result};

#[derive(Clone, Debug)]
pub struct StringEntry {
    pub index: u32,
    /// `None` if the entry is encrypted.
    pub text: Option<String>,
}

#[derive(Clone, Debug)]
pub struct StringsFile {
    pub language: u8,
    pub entries: Vec<StringEntry>,
}

pub fn language_name(code: u8) -> &'static str {
    match code {
        0 => "English",
        1 => "Korean",
        2 => "French",
        3 => "German",
        4 => "Spanish",
        5 => "Chinese",
        _ => "Unknown",
    }
}

/// Parses a `strs` file: entries of `[u16 size][u16 key offset][u16 bits per
/// symbol][UTF-16 text]`, then a 2-byte language trailer.
pub fn decode_strings(data: &[u8]) -> Result<StringsFile> {
    if data.len() < 6 || !data.starts_with(b"strs") {
        return Err(Error::format("strings file", "missing strs signature"));
    }
    let end = data.len() - 2;
    let language = data[end];
    let mut entries = Vec::new();
    let mut pos = 4;
    while pos < end {
        let header = data.get(pos..pos + 6).ok_or(Error::Truncated)?;
        let size = u16::from_le_bytes([header[0], header[1]]) as usize;
        let key_offset = u16::from_le_bytes([header[2], header[3]]);
        let bits = u16::from_le_bytes([header[4], header[5]]);
        if size < 6 || pos + size > end {
            return Err(Error::format("strings file", format!("bad entry size at {pos}")));
        }
        let text = (key_offset == 0 && bits == 0x10).then(|| {
            let units: Vec<u16> =
                data[pos + 6..pos + size].as_chunks::<2>().0.iter().map(|&b| u16::from_le_bytes(b)).collect();
            String::from_utf16_lossy(&units)
        });
        entries.push(StringEntry { index: entries.len() as u32, text });
        pos += size;
    }
    Ok(StringsFile { language, entries })
}

/// Renders a strings file as tab-separated `index<TAB>text` lines.
pub fn strings_to_tsv(file: &StringsFile) -> String {
    let mut out = String::from("index\ttext\n");
    for entry in &file.entries {
        if let Some(text) = &entry.text {
            let escaped = text.replace('\\', "\\\\").replace('\t', "\\t").replace('\r', "\\r").replace('\n', "\\n");
            out.push_str(&format!("{}\t{escaped}\n", entry.index));
        }
    }
    out
}

#[derive(Clone, Debug)]
pub struct EulaText {
    pub language: u8,
    pub text: String,
}

/// Extracts all language versions of a PF `eula` file.
pub fn decode_eula(data: &[u8]) -> Result<Vec<EulaText>> {
    let file = PackFile::parse(data)?;
    let chunk = file.require_chunk(pf::EULA)?;
    let (count, mut item) = chunk.cursor().array()?;
    let mut texts = Vec::with_capacity(count.min(64));
    for _ in 0..count {
        let language = item.u8()?;
        let text = item.wstring()?;
        texts.push(EulaText { language, text });
    }
    Ok(texts)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strings_file_with_plain_and_encrypted_entries() {
        let mut data = b"strs".to_vec();
        let text: Vec<u8> = "Hi".encode_utf16().flat_map(|u| u.to_le_bytes()).collect();
        data.extend_from_slice(&((6 + text.len()) as u16).to_le_bytes());
        data.extend_from_slice(&[0, 0, 0x10, 0]);
        data.extend_from_slice(&text);
        data.extend_from_slice(&[8, 0, 3, 0, 0x10, 0, 0xAB, 0xCD]); // encrypted
        data.extend_from_slice(&[3, 0]); // German
        let file = decode_strings(&data).unwrap();
        assert_eq!(language_name(file.language), "German");
        assert_eq!(file.entries[0].text.as_deref(), Some("Hi"));
        assert_eq!(file.entries[1].text, None);
        assert_eq!(strings_to_tsv(&file), "index\ttext\n0\tHi\n");
    }

    #[test]
    fn rejects_bad_entry_sizes() {
        assert!(decode_strings(b"strs\x02\x00\x00\x00\x10\x00\x00\x00").is_err());
    }
}
