//! Data files of the in-game web browser (Chromium Embedded Framework):
//! resource packs (`.pak`), the ICU Unicode data package and V8 snapshots.

use crate::error::{Error, Result};

fn u16_at(data: &[u8], at: usize) -> Result<u16> {
    data.get(at..at + 2).map(|b| u16::from_le_bytes([b[0], b[1]])).ok_or(Error::Truncated)
}

fn u32_at(data: &[u8], at: usize) -> Result<u32> {
    data.get(at..at + 4).map(|b| u32::from_le_bytes(b.try_into().unwrap())).ok_or(Error::Truncated)
}

#[derive(Clone, Debug)]
pub struct Pak {
    pub version: u32,
    /// 0 binary, 1 UTF-8, 2 UTF-16.
    pub encoding: u32,
    pub resources: Vec<PakResource>,
}

#[derive(Clone, Debug)]
pub struct PakResource {
    pub id: u16,
    pub offset: usize,
    pub size: usize,
    /// Resource this id is an alias of, if it is one.
    pub alias_of: Option<u16>,
}

/// A Chromium resource pack (versions 4 and 5).
pub fn pak(data: &[u8]) -> Result<Pak> {
    let version = u32_at(data, 0)?;
    let (encoding, count, aliases, table) = match version {
        5 => (u32_at(data, 4)?, u16_at(data, 8)? as usize, u16_at(data, 10)? as usize, 12),
        4 => (data.get(8).copied().ok_or(Error::Truncated)? as u32, u32_at(data, 4)? as usize, 0, 9),
        other => return Err(Error::Unsupported(format!("resource pack version {other}"))),
    };
    let entry = |i: usize| -> Result<(u16, usize)> {
        Ok((u16_at(data, table + i * 6)?, u32_at(data, table + i * 6 + 2)? as usize))
    };
    let mut resources = Vec::with_capacity(count + aliases);
    for i in 0..count {
        let (id, offset) = entry(i)?;
        let (_, end) = entry(i + 1)?;
        if offset > end || end > data.len() {
            return Err(Error::format("resource pack", "resource outside the file"));
        }
        resources.push(PakResource { id, offset, size: end - offset, alias_of: None });
    }
    let alias_table = table + (count + 1) * 6;
    for i in 0..aliases {
        let id = u16_at(data, alias_table + i * 4)?;
        let index = u16_at(data, alias_table + i * 4 + 2)? as usize;
        let target =
            resources.get(index).ok_or_else(|| Error::format("resource pack", "alias to a missing resource"))?.clone();
        resources.push(PakResource { id, alias_of: Some(target.id), ..target });
    }
    resources.sort_by_key(|r| r.id);
    Ok(Pak { version, encoding, resources })
}

/// What a resource most likely is, from its first bytes.
pub fn sniff(data: &[u8]) -> &'static str {
    let text = data.get(..512).unwrap_or(data);
    let start = String::from_utf8_lossy(text).trim_start().to_lowercase();
    match data {
        [0x1f, 0x8b, ..] => "gzip",
        [0x1e, 0x9b, ..] => "brotli",
        [0x89, b'P', b'N', b'G', ..] => "png",
        [0xff, 0xd8, 0xff, ..] => "jpeg",
        [b'G', b'I', b'F', b'8', ..] => "gif",
        [b'R', b'I', b'F', b'F', _, _, _, _, b'W', b'E', b'B', b'P', ..] => "webp",
        [b'w', b'O', b'F', b'F', ..] | [b'w', b'O', b'F', b'2', ..] => "font",
        _ if start.starts_with("<svg") || start.starts_with("<?xml") && start.contains("<svg") => "svg",
        _ if start.starts_with('<') => "html",
        _ if start.starts_with('{') || start.starts_with('[') => "json",
        _ if std::str::from_utf8(text).is_ok() || std::str::from_utf8(text).is_err_and(|e| e.error_len().is_none()) => {
            if crate::filetype::text_category(text) == "CSS" {
                "css"
            } else if crate::filetype::text_category(text) == "JavaScript" {
                "javascript"
            } else {
                "text"
            }
        }
        _ => "binary",
    }
}

/// Unpacks a gzip or Chromium brotli resource (`1e 9b`, a 6-byte size, then
/// the brotli stream). Other data is returned as `None`.
pub fn decompress(data: &[u8]) -> Result<Option<Vec<u8>>> {
    use std::io::Read;
    let mut out = Vec::new();
    match data {
        [0x1f, 0x8b, ..] => {
            flate2::read::GzDecoder::new(data)
                .read_to_end(&mut out)
                .map_err(|e| Error::format("gzip", e.to_string()))?;
        }
        [0x1e, 0x9b, rest @ ..] if rest.len() >= 6 => {
            let size = rest[..6].iter().rev().fold(0usize, |n, &b| n << 8 | b as usize);
            out.reserve(size.min(64 << 20));
            brotli_decompressor::Decompressor::new(&rest[6..], 4096)
                .read_to_end(&mut out)
                .map_err(|e| Error::format("brotli", e.to_string()))?;
        }
        _ => return Ok(None),
    }
    Ok(Some(out))
}

#[derive(Clone, Debug)]
pub struct IcuEntry {
    pub name: String,
    pub size: usize,
}

#[derive(Clone, Debug)]
pub struct IcuPackage {
    pub format_version: [u8; 4],
    pub data_version: [u8; 4],
    pub copyright: String,
    pub entries: Vec<IcuEntry>,
}

/// An ICU common data package (`CmnD`), e.g. `icudtl.dat`.
pub fn icu(data: &[u8]) -> Result<IcuPackage> {
    if data.get(2..4) != Some(&[0xda, 0x27]) || data.get(12..16) != Some(b"CmnD") {
        return Err(Error::format("ICU data", "not an ICU common data package"));
    }
    if data[8] != 0 {
        return Err(Error::Unsupported("big-endian ICU data".into()));
    }
    let header = u16_at(data, 0)? as usize;
    // The copyright follows the UDataInfo block, whose size comes first.
    let copyright = data.get(4 + u16_at(data, 4)? as usize..header).map_or(String::new(), |b| {
        let end = b.iter().position(|&c| c == 0).unwrap_or(b.len());
        String::from_utf8_lossy(&b[..end]).trim().to_owned()
    });
    let body = data.get(header..).ok_or(Error::Truncated)?;
    let count = u32_at(body, 0)? as usize;
    let mut offsets = Vec::with_capacity(count.min(1 << 16));
    for i in 0..count.min(1 << 16) {
        offsets.push((u32_at(body, 4 + i * 8)? as usize, u32_at(body, 8 + i * 8)? as usize));
    }
    let mut entries = Vec::with_capacity(offsets.len());
    for (i, &(name_at, data_at)) in offsets.iter().enumerate() {
        let rest = body.get(name_at..).ok_or(Error::Truncated)?;
        let end = rest.iter().position(|&c| c == 0).unwrap_or(rest.len());
        let next = offsets.get(i + 1).map_or(body.len(), |o| o.1);
        entries.push(IcuEntry {
            name: String::from_utf8_lossy(&rest[..end]).into_owned(),
            size: next.saturating_sub(data_at),
        });
    }
    Ok(IcuPackage {
        format_version: data[16..20].try_into().unwrap(),
        data_version: data[20..24].try_into().unwrap(),
        copyright,
        entries,
    })
}

#[derive(Clone, Debug)]
pub struct V8Snapshot {
    pub contexts: u32,
    pub rehashable: bool,
    pub checksum: u32,
    pub version: String,
}

/// Header of a V8 startup snapshot (`snapshot_blob.bin`, `v8_context_snapshot.bin`).
pub fn v8_snapshot(data: &[u8]) -> Result<V8Snapshot> {
    let version = data.get(12..76).ok_or(Error::Truncated)?;
    let end = version.iter().position(|&b| b == 0).unwrap_or(version.len());
    Ok(V8Snapshot {
        contexts: u32_at(data, 0)?,
        rehashable: u32_at(data, 4)? != 0,
        checksum: u32_at(data, 8)?,
        version: String::from_utf8_lossy(&version[..end]).into_owned(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pak_v5_with_alias() {
        let mut data = vec![5, 0, 0, 0, 1, 0, 0, 0, 2, 0, 1, 0];
        let start = 12 + 3 * 6 + 4;
        for (id, offset) in [(10u16, start), (11, start + 3), (0, start + 5)] {
            data.extend_from_slice(&id.to_le_bytes());
            data.extend_from_slice(&(offset as u32).to_le_bytes());
        }
        data.extend_from_slice(&[12, 0, 0, 0]); // id 12 -> resource 0
        data.extend_from_slice(b"abcde");
        let pak = pak(&data).unwrap();
        let sizes: Vec<(u16, usize, Option<u16>)> = pak.resources.iter().map(|r| (r.id, r.size, r.alias_of)).collect();
        assert_eq!(sizes, [(10, 3, None), (11, 2, None), (12, 3, Some(10))]);
        assert_eq!(&data[pak.resources[1].offset..][..2], b"de");
    }

    #[test]
    fn sniffing() {
        assert_eq!(sniff(b"\x89PNG\r\n"), "png");
        assert_eq!(sniff(b"  <!doctype html>"), "html");
        assert_eq!(sniff(b"{\"a\":1}"), "json");
        assert_eq!(sniff(&[0x1f, 0x8b, 8]), "gzip");
        assert_eq!(sniff(&[0x1e, 0x9b, 1]), "brotli");
        assert_eq!(sniff(&[0, 1, 2, 0xff]), "binary");
    }
}
