//! TextPack files: the index of all game texts (`txtm`), alternative
//! wordings (`txtV`) and the voice-over line of each text (`txtv`).
//!
//! Game texts live in strings files of `strings_per_file` entries each; text
//! id `n` is entry `n % strings_per_file` of file `n / strings_per_file` in a
//! language's file list.

use super::pf::PackFile;
use crate::error::Result;
use crate::fourcc::fcc;

#[derive(Clone, Debug)]
pub struct Manifest {
    pub strings_per_file: u32,
    /// Strings file ids per language (indexed like `text::language_name`).
    pub languages: Vec<Vec<Option<u32>>>,
}

impl Manifest {
    /// Strings file and entry index holding a text.
    pub fn locate(&self, language: usize, text_id: u32) -> Option<(u32, usize)> {
        let per_file = self.strings_per_file.max(1);
        let file = (*self.languages.get(language)?.get((text_id / per_file) as usize)?)?;
        Some((file, (text_id % per_file) as usize))
    }

    /// Number of text ids (the largest language).
    pub fn text_count(&self) -> u32 {
        self.languages.iter().map(|l| l.len() as u32).max().unwrap_or(0) * self.strings_per_file
    }
}

pub fn manifest(data: &[u8]) -> Result<Manifest> {
    let file = PackFile::parse(data)?;
    let mut c = file.require_chunk(fcc(b"txtm"))?.cursor();
    let strings_per_file = c.u32()?;
    let (count, mut language) = c.array()?;
    let mut languages = Vec::with_capacity(count.min(64));
    for _ in 0..count {
        let (files, mut item) = language.array()?;
        languages.push((0..files).map(|_| item.file_ref()).collect::<Result<Vec<_>>>()?);
    }
    Ok(Manifest { strings_per_file, languages })
}

/// `(text id, variant text ids)`.
pub fn variants(data: &[u8]) -> Result<Vec<(u32, Vec<u32>)>> {
    let file = PackFile::parse(data)?;
    let (count, mut item) = file.require_chunk(fcc(b"vari"))?.cursor().array()?;
    let mut out = Vec::with_capacity(count);
    for _ in 0..count {
        let text = item.u32()?;
        let (n, mut ids) = item.array()?;
        out.push((text, (0..n).map(|_| ids.u32()).collect::<Result<Vec<_>>>()?));
    }
    Ok(out)
}

/// `(text id, voice id)`; voice ids match sound bank entries.
pub fn voices(data: &[u8]) -> Result<Vec<(u32, u32)>> {
    let file = PackFile::parse(data)?;
    let (count, mut item) = file.require_chunk(fcc(b"txtv"))?.cursor().array()?;
    (0..count).map(|_| Ok((item.u32()?, item.u32()?))).collect()
}
