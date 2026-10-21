//! Scanning the archive into a browsable category tree, and caching the
//! result so that re-opening (even after a game patch) only scans what changed.

use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::{self, Read, Write};
use std::path::Path;
use std::sync::atomic::{AtomicU32, Ordering};

use rayon::prelude::*;

use crate::archive::{Archive, FIRST_FILE_ENTRY};
use crate::error::Result;
use crate::filetype::{self, FileType, Identified, PEEK_SIZE};
use crate::formats::model::{Classified, ModelInfo, classify};

#[derive(Clone, Debug)]
pub struct Category {
    pub name: String,
    pub parent: Option<u32>,
}

#[derive(Clone, Copy, Debug)]
pub struct IndexEntry {
    pub mft_index: u32,
    pub base_id: u32,
    pub file_id: u32,
    pub file_type: FileType,
    /// Decompressed size.
    pub size: u32,
    pub category: u32,
    /// Location of the stored data when the entry was scanned; a mismatch
    /// with the current MFT means the file changed.
    pub stored_offset: u64,
    pub stored_size: u32,
    /// Related file id: for models bound to an external skeleton, that
    /// skeleton's id; otherwise 0.
    pub reference: u32,
    /// For models: the file has playable animations of its own.
    pub has_clips: bool,
}

impl IndexEntry {
    /// Display name: the base id, or the MFT index for files without ids.
    pub fn name(&self) -> String {
        if self.base_id != 0 { self.base_id.to_string() } else { format!("ID-less_{}", self.mft_index) }
    }

    /// Original file name, for the few files where it is known.
    pub fn known_name(&self) -> Option<&'static str> {
        crate::names::known_name(self.base_id)
    }
}

#[derive(Clone, Debug, Default)]
pub struct Index {
    pub categories: Vec<Category>,
    pub entries: Vec<IndexEntry>,
}

/// Progress of a running scan, readable from other threads.
#[derive(Default)]
pub struct ScanProgress {
    pub done: AtomicU32,
    pub total: AtomicU32,
}

struct Scanned {
    mft_index: u32,
    file_type: FileType,
    size: u32,
    path: Vec<String>,
    reference: u32,
    has_clips: bool,
}

/// Sorts a model by its skeleton, animation, material and mesh data. Only the
/// file up to the `MODL`, `SKEL`, `ANIM` and `GEOM` chunk headers is
/// decompressed, not the geometry itself.
fn classify_model(archive: &Archive, index: u32, size: usize) -> ModelInfo {
    let mut len = (64 * 1024).min(size);
    loop {
        let Ok(prefix) = archive.peek(index, len) else { return ModelInfo::default() };
        match classify(&prefix, size) {
            Classified::Info(info) => return info,
            Classified::NeedBytes(needed) if prefix.len() < size => len = needed.max(len * 2).min(size),
            Classified::NeedBytes(_) => return ModelInfo::default(),
        }
    }
}

/// Models without clips of their own are animated if their skeleton file has
/// clips or animation sets were made for their skeleton, the same sources
/// the 3D viewer offers.
fn mark_inherited_animations(archive: &Archive, all: &mut [Scanned]) {
    let mut skeletons_with_clips = HashSet::new();
    for s in all.iter().filter(|s| s.file_type == FileType::ModelSkeleton && s.has_clips) {
        let ids = archive.ids(s.mft_index);
        skeletons_with_clips.extend([ids.base_id, ids.file_id].into_iter().filter(|&id| id != 0));
    }
    let set_targets: HashSet<u32> =
        all.iter().filter(|s| s.file_type == FileType::ModelAnimationSet).map(|s| s.reference).collect();
    for s in all.iter_mut().filter(|s| matches!(s.file_type, FileType::ModelAnimated | FileType::ModelStatic)) {
        let inherited = if s.reference != 0 {
            skeletons_with_clips.contains(&s.reference) || set_targets.contains(&s.reference)
        } else {
            let ids = archive.ids(s.mft_index);
            [ids.base_id, ids.file_id].iter().any(|id| *id != 0 && set_targets.contains(id))
        };
        let file_type = if s.has_clips || inherited { FileType::ModelAnimated } else { FileType::ModelStatic };
        s.file_type = file_type;
        s.path = file_type.category().iter().map(|s| s.to_string()).collect();
    }
}

fn scan_entry(archive: &Archive, index: u32) -> Result<Option<Scanned>> {
    let entry = archive.entry(index)?;
    if !entry.in_use() || entry.size == 0 {
        return Ok(None);
    }
    let (mut head, size) = archive.peek_sized(index, PEEK_SIZE)?;
    if size == 0 {
        return Ok(None);
    }
    let file_type = loop {
        let complete = head.len() >= size as usize;
        match filetype::identify(&head, complete) {
            Identified::Type(file_type) => break file_type,
            Identified::NeedMore(len) if len > head.len() => head = archive.peek(index, len)?,
            Identified::NeedMore(_) => break FileType::Unknown,
        }
    };
    let mut file_type = filetype::refine(file_type, &head, size);
    let (mut reference, mut has_clips) = (0, false);
    if file_type == FileType::Model {
        let info = classify_model(archive, index, size as usize);
        file_type = info.file_type();
        reference = info.skeleton.unwrap_or(0);
        has_clips = info.has_own_clips();
    }
    // Strings files store their language in the second-to-last byte.
    let tail = if file_type == FileType::Strings {
        let data = archive.read(index)?;
        data.len().checked_sub(2).map(|at| [data[at], data[at + 1]])
    } else {
        None
    };
    let path = filetype::categorize(file_type, &head, tail);
    Ok(Some(Scanned { mft_index: index, file_type, size, path, reference, has_clips }))
}

/// Glyph files of bitmap fonts have no signature; find them through the
/// references in the font files.
fn mark_font_glyphs(archive: &Archive, all: &mut [Scanned]) {
    let fonts: Vec<u32> = all.iter().filter(|s| s.file_type == FileType::BitmapFont).map(|s| s.mft_index).collect();
    let mut glyph_entries = std::collections::HashSet::new();
    for mft in fonts {
        let Ok(data) = archive.read(mft) else { continue };
        let Ok(ids) = crate::formats::font::referenced_files(&data) else { continue };
        glyph_entries.extend(ids.into_iter().filter_map(|id| archive.entry_for_id(id)));
    }
    for item in all.iter_mut().filter(|s| glyph_entries.contains(&s.mft_index)) {
        item.file_type = FileType::BitmapFontGlyphs;
        item.path = FileType::BitmapFontGlyphs.category().iter().map(|s| s.to_string()).collect();
    }
}

impl Index {
    /// Identifies every file in the archive, in parallel.
    pub fn scan(archive: &Archive, progress: &ScanProgress) -> Index {
        Index::update(archive, None, progress)
    }

    /// Like [`Index::scan`], but reuses everything from `previous` whose
    /// stored data did not move. Unreadable entries are listed under
    /// "Unreadable" instead of aborting the scan.
    pub fn update(archive: &Archive, previous: Option<&Index>, progress: &ScanProgress) -> Index {
        let mut known: HashMap<u32, (IndexEntry, Vec<String>)> = HashMap::new();
        if let Some(previous) = previous {
            for entry in &previous.entries {
                let unchanged = archive
                    .entry(entry.mft_index)
                    .is_ok_and(|e| e.in_use() && e.offset == entry.stored_offset && e.size == entry.stored_size);
                if unchanged {
                    known.insert(entry.mft_index, (*entry, previous.category_path(entry.category)));
                }
            }
        }

        let count = archive.entry_count();
        let pending: Vec<u32> = (FIRST_FILE_ENTRY..count).filter(|i| !known.contains_key(i)).collect();
        progress.total.store(pending.len() as u32, Ordering::Relaxed);
        progress.done.store(0, Ordering::Relaxed);

        let scanned: Vec<Scanned> = pending
            .into_par_iter()
            .with_min_len(256)
            .filter_map(|index| {
                let result = scan_entry(archive, index).unwrap_or_else(|_| {
                    Some(Scanned {
                        mft_index: index,
                        file_type: FileType::Unknown,
                        size: archive.entry(index).map_or(0, |e| e.size),
                        path: vec!["Unreadable".to_string()],
                        reference: 0,
                        has_clips: false,
                    })
                });
                progress.done.fetch_add(1, Ordering::Relaxed);
                result
            })
            .collect();

        let reused = known.into_values().map(|(e, path)| Scanned {
            mft_index: e.mft_index,
            file_type: e.file_type,
            size: e.size,
            path,
            reference: e.reference,
            has_clips: e.has_clips,
        });
        let mut all: Vec<Scanned> = scanned.into_iter().chain(reused).collect();
        all.sort_unstable_by_key(|s| s.mft_index);
        mark_font_glyphs(archive, &mut all);
        mark_inherited_animations(archive, &mut all);

        let mut index = Index::default();
        let mut lookup: HashMap<(Option<u32>, String), u32> = HashMap::new();
        for item in all {
            let mut parent = None;
            for name in item.path {
                let id = *lookup.entry((parent, name.clone())).or_insert_with(|| {
                    index.categories.push(Category { name, parent });
                    (index.categories.len() - 1) as u32
                });
                parent = Some(id);
            }
            let ids = archive.ids(item.mft_index);
            let stored = archive.entry(item.mft_index).ok();
            index.entries.push(IndexEntry {
                mft_index: item.mft_index,
                base_id: ids.base_id,
                file_id: ids.file_id,
                file_type: item.file_type,
                size: item.size,
                category: parent.expect("every path has at least one level"),
                stored_offset: stored.map_or(0, |e| e.offset),
                stored_size: stored.map_or(0, |e| e.size),
                reference: item.reference,
                has_clips: item.has_clips,
            });
        }
        index
    }

    /// Names from the root category down to `category`.
    pub fn category_path(&self, category: u32) -> Vec<String> {
        let mut path = Vec::new();
        let mut current = Some(category);
        while let Some(id) = current {
            let Some(c) = self.categories.get(id as usize) else { break };
            path.push(c.name.clone());
            current = c.parent;
        }
        path.reverse();
        path
    }
}

// --- Cache file -------------------------------------------------------------

const CACHE_MAGIC: &[u8; 8] = b"GW2BIDX\0";
/// Bump whenever file type detection or categorisation changes.
const CACHE_VERSION: u32 = 10;

impl Index {
    pub fn save(&self, path: &Path) -> io::Result<()> {
        let mut out = Vec::with_capacity(64 + self.entries.len() * 32);
        out.extend_from_slice(CACHE_MAGIC);
        out.extend_from_slice(&CACHE_VERSION.to_le_bytes());
        out.extend_from_slice(&(self.categories.len() as u32).to_le_bytes());
        out.extend_from_slice(&(self.entries.len() as u32).to_le_bytes());
        for c in &self.categories {
            out.extend_from_slice(&c.parent.unwrap_or(u32::MAX).to_le_bytes());
            out.extend_from_slice(&(c.name.len() as u16).to_le_bytes());
            out.extend_from_slice(c.name.as_bytes());
        }
        for e in &self.entries {
            out.extend_from_slice(&e.mft_index.to_le_bytes());
            out.extend_from_slice(&e.base_id.to_le_bytes());
            out.extend_from_slice(&e.file_id.to_le_bytes());
            let type_name = e.file_type.name().as_bytes();
            out.push(type_name.len() as u8);
            out.extend_from_slice(type_name);
            out.extend_from_slice(&e.size.to_le_bytes());
            out.extend_from_slice(&e.category.to_le_bytes());
            out.extend_from_slice(&e.stored_offset.to_le_bytes());
            out.extend_from_slice(&e.stored_size.to_le_bytes());
            out.extend_from_slice(&e.reference.to_le_bytes());
            out.push(e.has_clips as u8);
        }
        // Write to a temporary file first so a crash never leaves a broken cache.
        let tmp = path.with_extension("tmp");
        fs::File::create(&tmp)?.write_all(&out)?;
        fs::rename(tmp, path)
    }

    /// Loads a cache file; returns `None` if it is missing, from another
    /// version or damaged (the caller then simply scans).
    pub fn load(path: &Path) -> Option<Index> {
        let mut data = Vec::new();
        fs::File::open(path).ok()?.read_to_end(&mut data).ok()?;
        let mut r = Reader { data: &data, pos: 0 };
        if r.bytes(8)? != CACHE_MAGIC || r.u32()? != CACHE_VERSION {
            return None;
        }
        let (category_count, entry_count) = (r.u32()? as usize, r.u32()? as usize);
        let mut index =
            Index { categories: Vec::with_capacity(category_count), entries: Vec::with_capacity(entry_count) };
        for _ in 0..category_count {
            let parent = r.u32()?;
            let len = r.u16()? as usize;
            let name = String::from_utf8(r.bytes(len)?.to_vec()).ok()?;
            let parent = (parent != u32::MAX).then_some(parent);
            if parent.is_some_and(|p| p as usize >= category_count) {
                return None;
            }
            index.categories.push(Category { name, parent });
        }
        for _ in 0..entry_count {
            let (mft_index, base_id, file_id) = (r.u32()?, r.u32()?, r.u32()?);
            let len = r.bytes(1)?[0] as usize;
            let file_type = FileType::from_name(std::str::from_utf8(r.bytes(len)?).ok()?)?;
            let (size, category) = (r.u32()?, r.u32()?);
            if category as usize >= category_count {
                return None;
            }
            let stored_offset = u64::from_le_bytes(r.bytes(8)?.try_into().ok()?);
            let stored_size = r.u32()?;
            let reference = r.u32()?;
            let has_clips = r.bytes(1)?[0] != 0;
            index.entries.push(IndexEntry {
                mft_index,
                base_id,
                file_id,
                file_type,
                size,
                category,
                stored_offset,
                stored_size,
                reference,
                has_clips,
            });
        }
        Some(index)
    }
}

struct Reader<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    fn bytes(&mut self, n: usize) -> Option<&'a [u8]> {
        let slice = self.data.get(self.pos..self.pos.checked_add(n)?)?;
        self.pos += n;
        Some(slice)
    }
    fn u16(&mut self) -> Option<u16> {
        Some(u16::from_le_bytes(self.bytes(2)?.try_into().ok()?))
    }
    fn u32(&mut self) -> Option<u32> {
        Some(u32::from_le_bytes(self.bytes(4)?.try_into().ok()?))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cache_round_trip_and_rejection() {
        let index = Index {
            categories: vec![
                Category { name: "Textures".into(), parent: None },
                Category { name: "UI Textures".into(), parent: Some(0) },
            ],
            entries: vec![IndexEntry {
                mft_index: 20,
                base_id: 1234,
                file_id: 5678,
                file_type: FileType::Ateu,
                size: 99,
                category: 1,
                stored_offset: 0x1000,
                stored_size: 80,
                reference: 7,
                has_clips: true,
            }],
        };
        let path = std::env::temp_dir().join(format!("gw2dat-test-{}.bin", std::process::id()));
        index.save(&path).unwrap();
        let loaded = Index::load(&path).unwrap();
        assert_eq!(loaded.category_path(loaded.entries[0].category), ["Textures", "UI Textures"]);
        assert_eq!(loaded.entries[0].file_type, FileType::Ateu);
        assert_eq!(loaded.entries[0].stored_offset, 0x1000);
        assert_eq!(loaded.entries[0].reference, 7);
        assert!(loaded.entries[0].has_clips);

        // Damaged files are ignored rather than trusted.
        let mut bytes = std::fs::read(&path).unwrap();
        bytes.truncate(bytes.len() - 3);
        std::fs::write(&path, &bytes).unwrap();
        assert!(Index::load(&path).is_none());
        std::fs::remove_file(path).unwrap();
    }
}
