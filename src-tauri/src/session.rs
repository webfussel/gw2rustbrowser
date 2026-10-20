//! An opened archive together with its index and lookup tables.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use gw2dat::{Archive, FileType, Index, IndexEntry};

pub struct Session {
    pub archive: Archive,
    pub index: Index,
    /// Child categories per category.
    pub children: Vec<Vec<u32>>,
    /// Entries (indices into `index.entries`) directly in each category.
    pub files: Vec<Vec<u32>>,
    /// Number of files in each category including all subcategories.
    pub totals: Vec<u32>,
    /// Most common file type in each category including subcategories.
    pub dominant: Vec<Option<FileType>>,
    by_mft: HashMap<u32, u32>,
    /// The most recently read file; previews read the same file repeatedly.
    recent: Mutex<Option<(u32, Arc<Vec<u8>>)>>,
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
        let dominant = type_counts
            .iter()
            .map(|counts| counts.iter().max_by_key(|&(t, n)| (*n, std::cmp::Reverse(*t))).map(|(&t, _)| t))
            .collect();
        Session { archive, index, children, files, totals, dominant, by_mft, recent: Mutex::new(None) }
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
