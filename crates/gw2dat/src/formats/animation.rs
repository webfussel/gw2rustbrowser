//! Animation-only model files: a PF `MODL` file with a single `ANIM` chunk
//! that carries one Granny 3D animation clip (in-memory layout plus a pointer
//! relocation table) and no animation bank.

use super::pf::PackFile;
use crate::error::{Error, Result};
use crate::fourcc::fcc;

pub const ANIM: u32 = fcc(b"ANIM");

#[derive(Clone, Debug)]
pub struct AnimationInfo {
    /// Clip name, e.g. `ugmrun    r`.
    pub name: String,
    /// Length in seconds.
    pub duration: f32,
    /// Seconds per sampled frame.
    pub time_step: f32,
    pub oversampling: f32,
    pub track_groups: u32,
    pub wide_pointers: bool,
    pub chunk_version: u16,
    /// Size of the Granny data block.
    pub data_size: usize,
    /// Entries of the pointer relocation table.
    pub relocations: usize,
}

impl AnimationInfo {
    pub fn frames(&self) -> u32 {
        if self.time_step > 0.0 { (self.duration / self.time_step).round() as u32 } else { 0 }
    }
}

/// True for model files that contain nothing but one `ANIM` chunk. Needs only
/// the first 20 bytes and the decompressed file size.
pub fn is_animation_only(head: &[u8], file_size: u32) -> bool {
    let (Some(magic), Some(next)) = (head.get(12..16), head.get(16..20)) else { return false };
    let next = u32::from_le_bytes(next.try_into().unwrap()) as u64;
    magic == b"ANIM" && 12 + 8 + next == file_size as u64
}

pub fn info(data: &[u8]) -> Result<AnimationInfo> {
    let file = PackFile::parse(data)?;
    let chunk = file.require_chunk(ANIM)?;
    let mut c = chunk.cursor();
    if c.ptr()?.is_some() {
        return Err(Error::Unsupported("animation banks".into()));
    }
    let blob = c.byte_array()?;
    let (relocations, _) = c.array()?;

    // granny_animation: Name*, Duration, TimeStep, Oversampling, TrackGroupCount, TrackGroups**
    let wide = file.wide_pointers();
    let pointer = if wide { 8 } else { 4 };
    let u32_at =
        |at: usize| blob.get(at..at + 4).map(|b| u32::from_le_bytes(b.try_into().unwrap())).ok_or(Error::Truncated);
    let name_offset = if wide {
        blob.get(0..8).map(|b| u64::from_le_bytes(b.try_into().unwrap()) as usize).ok_or(Error::Truncated)?
    } else {
        u32_at(0)? as usize
    };
    let name = blob
        .get(name_offset..)
        .map(|rest| {
            let end = rest.iter().take(256).position(|&b| b == 0).unwrap_or(0);
            String::from_utf8_lossy(&rest[..end]).trim_end().to_string()
        })
        .ok_or_else(|| Error::format("animation", "name outside of the data"))?;

    Ok(AnimationInfo {
        name,
        duration: f32::from_bits(u32_at(pointer)?),
        time_step: f32::from_bits(u32_at(pointer + 4)?),
        oversampling: f32::from_bits(u32_at(pointer + 8)?),
        track_groups: u32_at(pointer + 12)?,
        wide_pointers: wide,
        chunk_version: chunk.version,
        data_size: blob.len(),
        relocations,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_anim_chunk_spanning_the_file() {
        let mut head = b"PF\x01\x00\x00\x00\x0c\x00MODLANIM".to_vec();
        head.extend_from_slice(&100u32.to_le_bytes());
        assert!(is_animation_only(&head, 120));
        assert!(!is_animation_only(&head, 500)); // more chunks follow
        head[12..16].copy_from_slice(b"MODL");
        assert!(!is_animation_only(&head, 120));
    }
}
