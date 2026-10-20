//! The Gw2.dat container: header, master file table (MFT) and id mapping.

use std::collections::HashMap;
use std::fs::File;
use std::io;
use std::path::{Path, PathBuf};

use crate::error::{Error, Result};
use crate::inflate;

const HEADER_SIZE: usize = 40;
const MFT_ENTRY_SIZE: usize = 24;
const MFT_MAGIC: &[u8; 4] = b"Mft\x1a";
/// MFT index of the table mapping file ids to MFT entries.
const ID_TABLE_ENTRY: usize = 2;
/// The first MFT entries describe the archive itself, not game files.
pub const FIRST_FILE_ENTRY: u32 = 16;

const FLAG_COMPRESSED: u16 = 0x08;
const FLAG_IN_USE: u16 = 0x01;

/// Uncompressed entries are stored in 64 KiB blocks whose last 4 bytes are a
/// checksum.
const RAW_BLOCK: usize = 0x10000;
const RAW_BLOCK_DATA: usize = RAW_BLOCK - 4;

#[derive(Clone, Copy, Debug)]
pub struct DatHeader {
    pub version: u8,
    pub chunk_size: u32,
    pub mft_offset: u64,
    pub mft_size: u32,
}

#[derive(Clone, Copy, Debug)]
pub struct MftEntry {
    pub offset: u64,
    /// Size of the stored (possibly compressed) data.
    pub size: u32,
    pub compression_flag: u16,
    pub entry_flags: u16,
}

impl MftEntry {
    pub fn is_compressed(&self) -> bool {
        self.compression_flag & FLAG_COMPRESSED != 0
    }

    pub fn in_use(&self) -> bool {
        self.entry_flags & FLAG_IN_USE != 0
    }
}

/// Ids pointing at one MFT entry. Most entries have a base id (the stable
/// id of the asset) and a file id (its current revision); 0 means "none".
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct EntryIds {
    pub base_id: u32,
    pub file_id: u32,
}

impl EntryIds {
    /// The id the game uses to reference the file's current content.
    pub fn primary(&self) -> u32 {
        if self.file_id != 0 { self.file_id } else { self.base_id }
    }
}

pub struct Archive {
    file: File,
    path: PathBuf,
    file_size: u64,
    header: DatHeader,
    entries: Vec<MftEntry>,
    ids: Vec<EntryIds>,
    by_id: HashMap<u32, u32>,
}

fn read_at(file: &File, buf: &mut [u8], offset: u64) -> io::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::FileExt;
        file.read_exact_at(buf, offset)
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::FileExt;
        let mut done = 0;
        while done < buf.len() {
            match file.seek_read(&mut buf[done..], offset + done as u64) {
                Ok(0) => return Err(io::ErrorKind::UnexpectedEof.into()),
                Ok(n) => done += n,
                Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
                Err(e) => return Err(e),
            }
        }
        Ok(())
    }
}

fn le_u16(b: &[u8], at: usize) -> u16 {
    u16::from_le_bytes(b[at..at + 2].try_into().unwrap())
}
fn le_u32(b: &[u8], at: usize) -> u32 {
    u32::from_le_bytes(b[at..at + 4].try_into().unwrap())
}
fn le_u64(b: &[u8], at: usize) -> u64 {
    u64::from_le_bytes(b[at..at + 8].try_into().unwrap())
}

impl Archive {
    pub fn open(path: impl AsRef<Path>) -> Result<Archive> {
        let path = path.as_ref().to_path_buf();
        let file = File::open(&path)?;
        let file_size = file.metadata()?.len();
        let invalid = |reason: &str| Error::InvalidArchive(reason.to_string());

        if file_size < HEADER_SIZE as u64 {
            return Err(invalid("file is too small"));
        }
        let mut raw = [0u8; HEADER_SIZE];
        read_at(&file, &mut raw, 0)?;
        if &raw[1..4] != b"AN\x1a" {
            return Err(invalid("missing 'AN' signature"));
        }
        let header = DatHeader {
            version: raw[0],
            chunk_size: le_u32(&raw, 12),
            mft_offset: le_u64(&raw, 24),
            mft_size: le_u32(&raw, 32),
        };

        let mft_size = header.mft_size as usize;
        if header.mft_offset.checked_add(mft_size as u64).is_none_or(|end| end > file_size) {
            return Err(invalid("MFT lies outside the file"));
        }
        if !mft_size.is_multiple_of(MFT_ENTRY_SIZE) || mft_size < MFT_ENTRY_SIZE * FIRST_FILE_ENTRY as usize {
            return Err(invalid("MFT has an invalid size"));
        }
        let mut mft = vec![0u8; mft_size];
        read_at(&file, &mut mft, header.mft_offset)?;
        if &mft[0..4] != MFT_MAGIC {
            return Err(invalid("missing MFT signature"));
        }
        let declared = le_u32(&mft, 12) as usize;
        if declared * MFT_ENTRY_SIZE != mft_size {
            return Err(invalid("MFT entry count does not match its size"));
        }

        let entries: Vec<MftEntry> = mft
            .as_chunks::<MFT_ENTRY_SIZE>()
            .0
            .iter()
            .map(|e| MftEntry {
                offset: le_u64(e, 0),
                size: le_u32(e, 8),
                compression_flag: le_u16(e, 12),
                entry_flags: le_u16(e, 14),
            })
            .collect();

        let mut archive = Archive { file, path, file_size, header, entries, ids: Vec::new(), by_id: HashMap::new() };
        archive.read_id_table()?;
        Ok(archive)
    }

    fn read_id_table(&mut self) -> Result<()> {
        let table = self.read_raw(ID_TABLE_ENTRY as u32)?;
        let mut ids = vec![EntryIds::default(); self.entries.len()];
        let mut by_id = HashMap::with_capacity(table.len() / 8);

        for pair in table.as_chunks::<8>().0 {
            let (id, index) = (le_u32(pair, 0), le_u32(pair, 4));
            if id == 0 && index == 0 {
                continue;
            }
            let Some(slot) = ids.get_mut(index as usize) else { continue };
            by_id.insert(id, index);
            if slot.base_id == 0 {
                slot.base_id = id;
            } else if slot.file_id == 0 {
                slot.file_id = id;
            }
            if slot.base_id != 0 && slot.file_id != 0 && slot.base_id > slot.file_id {
                std::mem::swap(&mut slot.base_id, &mut slot.file_id);
            }
        }
        self.ids = ids;
        self.by_id = by_id;
        Ok(())
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn file_size(&self) -> u64 {
        self.file_size
    }

    pub fn header(&self) -> &DatHeader {
        &self.header
    }

    pub fn entry_count(&self) -> u32 {
        self.entries.len() as u32
    }

    pub fn entry(&self, index: u32) -> Result<&MftEntry> {
        self.entries.get(index as usize).ok_or(Error::NoSuchEntry(index))
    }

    pub fn ids(&self, index: u32) -> EntryIds {
        self.ids.get(index as usize).copied().unwrap_or_default()
    }

    /// Finds the MFT entry for a base or file id.
    pub fn entry_for_id(&self, id: u32) -> Option<u32> {
        self.by_id.get(&id).copied()
    }

    fn checked_entry(&self, index: u32) -> Result<MftEntry> {
        let entry = *self.entry(index)?;
        if !entry.in_use() {
            return Err(Error::UnusedEntry(index));
        }
        let end = entry.offset.checked_add(entry.size as u64);
        if end.is_none_or(|end| end > self.file_size) {
            return Err(Error::InvalidArchive(format!("entry {index} lies outside the file")));
        }
        Ok(entry)
    }

    fn read_stored(&self, entry: &MftEntry, len: usize) -> Result<Vec<u8>> {
        let mut buf = vec![0u8; len.min(entry.size as usize)];
        read_at(&self.file, &mut buf, entry.offset)?;
        Ok(buf)
    }

    /// The bytes as stored in the archive (still compressed, with checksums).
    pub fn read_raw(&self, index: u32) -> Result<Vec<u8>> {
        let entry = self.checked_entry(index)?;
        self.read_stored(&entry, entry.size as usize)
    }

    /// Size of the file content once decompressed.
    pub fn file_size_of(&self, index: u32) -> Result<u32> {
        let entry = self.checked_entry(index)?;
        if entry.is_compressed() {
            inflate::inflated_size(&self.read_stored(&entry, 8)?)
        } else {
            Ok(strip_checksums_len(entry.size as usize) as u32)
        }
    }

    /// Reads and decompresses a whole file.
    pub fn read(&self, index: u32) -> Result<Vec<u8>> {
        let entry = self.checked_entry(index)?;
        let stored = self.read_stored(&entry, entry.size as usize)?;
        if entry.is_compressed() { inflate::inflate(&stored, None) } else { Ok(strip_checksums(&stored)) }
    }

    /// Reads at most `len` bytes from the start of a file. For compressed
    /// files only a small part of the stored data is read and decoded.
    pub fn peek(&self, index: u32, len: usize) -> Result<Vec<u8>> {
        self.peek_sized(index, len).map(|(data, _)| data)
    }

    /// Like [`Archive::peek`], also returning the full decompressed size.
    pub fn peek_sized(&self, index: u32, len: usize) -> Result<(Vec<u8>, u32)> {
        let entry = self.checked_entry(index)?;
        if !entry.is_compressed() {
            // Read whole blocks so the checksum position is known.
            let blocks = len.div_ceil(RAW_BLOCK_DATA).max(1);
            let mut data = strip_checksums(&self.read_stored(&entry, blocks * RAW_BLOCK)?);
            data.truncate(len);
            return Ok((data, strip_checksums_len(entry.size as usize) as u32));
        }

        // Huffman tables come first; a few KiB of input cover them and the
        // first bytes of output for practically every file.
        let mut input_len = 4096usize;
        loop {
            let input = self.read_stored(&entry, input_len)?;
            let complete = input.len() >= entry.size as usize;
            match inflate::inflate(&input, Some(len as u32)) {
                Err(Error::Truncated) if !complete => input_len = input_len.saturating_mul(8),
                result => return Ok((result?, inflate::inflated_size(&input)?)),
            }
        }
    }
}

fn strip_checksums_len(stored: usize) -> usize {
    stored - stored.div_ceil(RAW_BLOCK) * 4
}

/// Removes the 4-byte checksum that ends every 64 KiB block of stored data,
/// including the last, partial block.
fn strip_checksums(stored: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(stored.len());
    for block in stored.chunks(RAW_BLOCK) {
        out.extend_from_slice(&block[..block.len().saturating_sub(4)]);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_checksum_of_every_block_including_the_last() {
        let mut stored = vec![1u8; RAW_BLOCK_DATA];
        stored.extend_from_slice(&[0xEE; 4]);
        stored.extend_from_slice(&[2, 3, 4]);
        stored.extend_from_slice(&[0xEE; 4]);
        let data = strip_checksums(&stored);
        assert_eq!(data.len(), RAW_BLOCK_DATA + 3);
        assert_eq!(strip_checksums_len(stored.len()), data.len());
        assert_eq!(&data[RAW_BLOCK_DATA..], &[2, 3, 4]);
        assert!(!data.contains(&0xEE));
    }

    #[test]
    fn primary_id_prefers_file_id() {
        assert_eq!(EntryIds { base_id: 5, file_id: 9 }.primary(), 9);
        assert_eq!(EntryIds { base_id: 5, file_id: 0 }.primary(), 5);
    }
}
