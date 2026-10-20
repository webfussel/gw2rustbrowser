//! An opened archive together with its index and lookup tables.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use gw2dat::{Archive, FileType, Index, IndexEntry, Kind};

pub struct Session {
    pub archive: Archive,
    pub index: Index,
    /// Child categories per category.
    pub children: Vec<Vec<u32>>,
    /// Entries (indices into `index.entries`) directly in each category.
    pub files: Vec<Vec<u32>>,
    /// Number of files in each category including all subcategories.
    pub totals: Vec<u32>,
    /// Representative file type of each category (including subcategories),
    /// and whether the category mixes several types.
    pub content: Vec<Option<(FileType, bool)>>,
    by_mft: HashMap<u32, u32>,
    /// Number of models bound to each skeleton id.
    pub skeleton_users: HashMap<u32, u32>,
    /// The most recently read file; previews read the same file repeatedly.
    recent: Mutex<Option<(u32, Arc<Vec<u8>>)>>,
}

/// For mixed categories, the most common type of the most common kind,
/// preferring any viewable kind over plain binary data.
fn representative(counts: &HashMap<FileType, u32>) -> Option<(FileType, bool)> {
    let mut kinds: HashMap<Kind, u32> = HashMap::new();
    for (t, &n) in counts {
        *kinds.entry(t.kind()).or_default() += n;
    }
    let kind = kinds.iter().max_by_key(|&(k, n)| (*k != Kind::Binary, *n)).map(|(&k, _)| k)?;
    let file_type = counts
        .iter()
        .filter(|(t, _)| t.kind() == kind)
        .max_by_key(|&(t, n)| (*n, std::cmp::Reverse(*t)))
        .map(|(&t, _)| t)?;
    Some((file_type, counts.len() > 1))
}

impl Session {
    pub fn new(archive: Archive, index: Index) -> Session {
        let count = index.categories.len();
        let mut children = vec![Vec::new(); count];
        for (id, category) in index.categories.iter().enumerate() {
            if let Some(parent) = category.parent {
                children[parent as usize].push(id as u32);
            }
        }
        let mut files = vec![Vec::new(); count];
        let mut by_mft = HashMap::with_capacity(index.entries.len());
        for (i, entry) in index.entries.iter().enumerate() {
            files[entry.category as usize].push(i as u32);
            by_mft.insert(entry.mft_index, i as u32);
        }
        let mut totals = vec![0u32; count];
        let mut type_counts: Vec<HashMap<FileType, u32>> = vec![HashMap::new(); count];
        for (id, list) in files.iter().enumerate() {
            let mut direct: HashMap<FileType, u32> = HashMap::new();
            for &i in list {
                *direct.entry(index.entries[i as usize].file_type).or_default() += 1;
            }
            let mut current = Some(id as u32);
            while let Some(c) = current {
                totals[c as usize] += list.len() as u32;
                for (&t, &n) in &direct {
                    *type_counts[c as usize].entry(t).or_default() += n;
                }
                current = index.categories[c as usize].parent;
            }
        }
        let content = type_counts.iter().map(representative).collect();
        let mut skeleton_users: HashMap<u32, u32> = HashMap::new();
        for e in index.entries.iter().filter(|e| e.reference != 0) {
            *skeleton_users.entry(e.reference).or_default() += 1;
        }
        Session { archive, index, children, files, totals, content, by_mft, skeleton_users, recent: Mutex::new(None) }
    }

    pub fn entry(&self, mft_index: u32) -> Option<&IndexEntry> {
        self.by_mft.get(&mft_index).map(|&i| &self.index.entries[i as usize])
    }

    /// Decompressed content of a file, cached for repeated access.
    pub fn read(&self, mft_index: u32) -> gw2dat::Result<Arc<Vec<u8>>> {
        if let Some((cached, data)) = &*self.recent.lock().unwrap()
            && *cached == mft_index
        {
            return Ok(data.clone());
        }
        let data = Arc::new(self.archive.read(mft_index)?);
        *self.recent.lock().unwrap() = Some((mft_index, data.clone()));
        Ok(data)
    }

    /// All entries in a category and its subcategories.
    pub fn files_recursive(&self, category: u32) -> Vec<u32> {
        let mut out = Vec::new();
        let mut stack = vec![category];
        while let Some(c) = stack.pop() {
            out.extend_from_slice(&self.files[c as usize]);
            stack.extend_from_slice(&self.children[c as usize]);
        }
        out
    }
}
