//! ArenaNet's "PF" container: a header followed by typed, versioned chunks.
//!
//! Chunk payloads are packed structs whose pointers are signed offsets
//! relative to the pointer field itself. Files with header flag `0x4` use
//! 64-bit offsets, older ones 32-bit (gw2formats only knows the latter).

use crate::error::{Error, Result};
use crate::fourcc::{self, fcc};

const HEADER_SIZE: usize = 12;
const CHUNK_HEADER_SIZE: usize = 16;
const FLAG_WIDE_POINTERS: u16 = 0x4;

#[derive(Clone, Copy)]
pub struct PackFile<'a> {
    data: &'a [u8],
    pub flags: u16,
    pub file_type: u32,
}

#[derive(Clone, Copy)]
pub struct Chunk<'a> {
    file: &'a [u8],
    pub magic: u32,
    pub version: u16,
    /// Offset of the chunk payload in the file.
    pub offset: usize,
    pub size: usize,
    wide: bool,
}

impl<'a> PackFile<'a> {
    pub fn parse(data: &'a [u8]) -> Result<Self> {
        if data.len() < HEADER_SIZE || &data[..2] != b"PF" {
            return Err(Error::format("PF file", "missing PF signature"));
        }
        Ok(PackFile {
            data,
            flags: u16::from_le_bytes([data[2], data[3]]),
            file_type: u32::from_le_bytes(data[8..12].try_into().unwrap()),
        })
    }

    pub fn wide_pointers(&self) -> bool {
        self.flags & FLAG_WIDE_POINTERS != 0
    }

    pub fn chunks(&self) -> impl Iterator<Item = Chunk<'a>> + 'a {
        let (data, wide) = (self.data, self.wide_pointers());
        let mut pos = HEADER_SIZE;
        std::iter::from_fn(move || {
            let header = data.get(pos..pos + CHUNK_HEADER_SIZE)?;
            let magic = u32::from_le_bytes(header[0..4].try_into().unwrap());
            let next = u32::from_le_bytes(header[4..8].try_into().unwrap()) as usize;
            let version = u16::from_le_bytes([header[8], header[9]]);
            let header_size = u16::from_le_bytes([header[10], header[11]]) as usize;
            let total = next.checked_add(8)?;
            let end = pos.checked_add(total)?.min(data.len());
            let chunk = Chunk {
                file: data,
                magic,
                version,
                offset: pos + CHUNK_HEADER_SIZE,
                size: end.saturating_sub(pos + header_size.max(CHUNK_HEADER_SIZE)),
                wide,
            };
            pos += total.max(CHUNK_HEADER_SIZE);
            Some(chunk)
        })
    }

    pub fn chunk(&self, magic: u32) -> Option<Chunk<'a>> {
        self.chunks().find(|c| c.magic == magic)
    }

    pub fn require_chunk(&self, magic: u32) -> Result<Chunk<'a>> {
        self.chunk(magic).ok_or_else(|| Error::format("PF file", format!("no {} chunk", fourcc::display(magic))))
    }
}

impl<'a> Chunk<'a> {
    /// Cursor at the start of the chunk payload.
    pub fn cursor(&self) -> Cursor<'a> {
        Cursor { file: self.file, pos: self.offset, wide: self.wide }
    }

    pub fn bytes(&self) -> &'a [u8] {
        &self.file[self.offset..(self.offset + self.size).min(self.file.len())]
    }
}

/// Reads packed struct fields; pointers may lead anywhere in the file.
#[derive(Clone, Copy)]
pub struct Cursor<'a> {
    file: &'a [u8],
    pub pos: usize,
    wide: bool,
}

fn out_of_bounds() -> Error {
    Error::format("PF file", "field outside of the file")
}

impl<'a> Cursor<'a> {
    pub fn take(&mut self, n: usize) -> Result<&'a [u8]> {
        let bytes =
            self.file.get(self.pos..self.pos.checked_add(n).ok_or_else(out_of_bounds)?).ok_or_else(out_of_bounds)?;
        self.pos += n;
        Ok(bytes)
    }

    pub fn skip(&mut self, n: usize) -> Result<()> {
        self.take(n).map(|_| ())
    }

    pub fn u8(&mut self) -> Result<u8> {
        Ok(self.take(1)?[0])
    }
    pub fn u16(&mut self) -> Result<u16> {
        Ok(u16::from_le_bytes(self.take(2)?.try_into().unwrap()))
    }
    pub fn u32(&mut self) -> Result<u32> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }
    pub fn u64(&mut self) -> Result<u64> {
        Ok(u64::from_le_bytes(self.take(8)?.try_into().unwrap()))
    }
    pub fn f32(&mut self) -> Result<f32> {
        Ok(f32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }

    pub fn pointer_size(&self) -> usize {
        if self.wide { 8 } else { 4 }
    }

    /// Reads a relative offset field and returns the absolute target.
    fn offset(&mut self) -> Result<Option<usize>> {
        let field = self.pos;
        let offset = if self.wide { self.u64()? as i64 } else { self.u32()? as i32 as i64 };
        if offset == 0 {
            return Ok(None);
        }
        let target = field as i64 + offset;
        if target < 0 || target as usize >= self.file.len() {
            return Err(out_of_bounds());
        }
        Ok(Some(target as usize))
    }

    fn at(&self, pos: usize) -> Cursor<'a> {
        Cursor { file: self.file, pos, wide: self.wide }
    }

    /// `Ptr<T>`: cursor at the pointed-to struct, if not null.
    pub fn ptr(&mut self) -> Result<Option<Cursor<'a>>> {
        Ok(self.offset()?.map(|p| self.at(p)))
    }

    /// `Array<T>`: element count and a cursor at the first element.
    pub fn array(&mut self) -> Result<(usize, Cursor<'a>)> {
        let count = self.u32()? as usize;
        let target = self.offset()?;
        match target {
            Some(p) => Ok((count, self.at(p))),
            None if count == 0 => Ok((0, self.at(self.pos))),
            None => Err(Error::format("PF file", "null array with elements")),
        }
    }

    /// `Array<byte>`.
    pub fn byte_array(&mut self) -> Result<&'a [u8]> {
        let (count, mut at) = self.array()?;
        at.take(count)
    }

    /// `RefList<T>`: array of pointers; returns a cursor per non-null element.
    pub fn ref_list(&mut self) -> Result<Vec<Cursor<'a>>> {
        let (count, mut at) = self.array()?;
        let mut items = Vec::with_capacity(count.min(1 << 16));
        for _ in 0..count {
            if let Some(item) = at.ptr()? {
                items.push(item);
            }
        }
        Ok(items)
    }

    /// `WString`: pointer to a zero-terminated UTF-16 string.
    pub fn wstring(&mut self) -> Result<String> {
        let Some(mut at) = self.ptr()? else { return Ok(String::new()) };
        let mut units = Vec::new();
        loop {
            match at.u16()? {
                0 => break,
                unit => units.push(unit),
            }
        }
        Ok(String::from_utf16_lossy(&units))
    }

    /// `String`: pointer to a zero-terminated 8-bit string.
    pub fn string(&mut self) -> Result<String> {
        let Some(at) = self.ptr()? else { return Ok(String::new()) };
        let rest = &self.file[at.pos..];
        let end = rest.iter().position(|&b| b == 0).unwrap_or(rest.len());
        Ok(String::from_utf8_lossy(&rest[..end]).into_owned())
    }

    /// `FileName`: pointer to a 3×u16 file reference; returns the file id.
    pub fn file_ref(&mut self) -> Result<Option<u32>> {
        let Some(mut at) = self.ptr()? else { return Ok(None) };
        let (a, b, _c) = (at.u16()?, at.u16()?, at.u16()?);
        Ok(file_id_from_reference(a, b))
    }
}

/// Converts the two significant parts of an ANet file reference to a file id.
pub fn file_id_from_reference(low: u16, high: u16) -> Option<u32> {
    if low < 0x100 || high < 0x100 {
        return None;
    }
    Some(0xff00 * (high as u32 - 0x100) + (low as u32 - 0x100) + 1)
}

/// PF type of a file, if it is a PF file.
pub fn pf_type(data: &[u8]) -> Option<u32> {
    PackFile::parse(data).ok().map(|p| p.file_type)
}

pub const ASND: u32 = fcc(b"ASND");
pub const ABNK: u32 = fcc(b"ABNK");
pub const BKCK: u32 = fcc(b"BKCK");
pub const EULA: u32 = fcc(b"eula");

#[cfg(test)]
mod tests {
    use super::*;

    /// PF file with one chunk whose payload is `payload`.
    fn pf(flags: u16, payload: &[u8]) -> Vec<u8> {
        let mut data = b"PF".to_vec();
        data.extend_from_slice(&flags.to_le_bytes());
        data.extend_from_slice(&[0, 0, 12, 0]);
        data.extend_from_slice(b"TEST");
        data.extend_from_slice(b"CHNK");
        data.extend_from_slice(&((payload.len() + 8) as u32).to_le_bytes());
        data.extend_from_slice(&[1, 0, 16, 0, 0, 0, 0, 0]);
        data.extend_from_slice(payload);
        data
    }

    #[test]
    fn arrays_with_32_and_64_bit_offsets() {
        // count 2, offset 4 (relative to the offset field) -> elements follow.
        let narrow = pf(1, &[2, 0, 0, 0, 4, 0, 0, 0, 0xAA, 0xBB]);
        let wide = pf(5, &[2, 0, 0, 0, 8, 0, 0, 0, 0, 0, 0, 0, 0xAA, 0xBB]);
        for data in [narrow, wide] {
            let file = PackFile::parse(&data).unwrap();
            let chunk = file.chunk(fcc(b"CHNK")).unwrap();
            assert_eq!(chunk.version, 1);
            assert_eq!(chunk.cursor().byte_array().unwrap(), &[0xAA, 0xBB]);
        }
    }

    #[test]
    fn pointers_outside_the_file_are_errors() {
        let data = pf(1, &[0xff, 0xff, 0, 0]);
        let mut cursor = PackFile::parse(&data).unwrap().chunk(fcc(b"CHNK")).unwrap().cursor();
        assert!(cursor.ptr().is_err());
    }

    #[test]
    fn file_references() {
        assert_eq!(file_id_from_reference(0x100, 0x100), Some(1));
        assert_eq!(file_id_from_reference(0x101, 0x101), Some(0xff00 + 2));
        assert_eq!(file_id_from_reference(0x50, 0x100), None);
    }
}
