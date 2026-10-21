//! Model skeletons (Granny skeletons inside the `SKEL` chunk) and the name
//! tokens meshes use to refer to bones.

use super::pf::PackFile;
use crate::error::Result;
use crate::fourcc::fcc;

pub const SKEL: u32 = fcc(b"SKEL");

#[derive(Clone, Debug)]
pub struct Bone {
    /// Full name, e.g. `bone:Spine02` or `actionpoint:Aura`.
    pub name: String,
    /// Index of the parent bone, `None` for roots.
    pub parent: Option<usize>,
    /// Rest transform relative to the parent.
    pub translation: [f32; 3],
    /// Quaternion `x, y, z, w`.
    pub rotation: [f32; 4],
    /// Row-major 3x3 scale/shear matrix.
    pub scale_shear: [f32; 9],
}

/// The model's own skeleton (empty if it has none or only one bone).
pub fn bones(data: &[u8]) -> Result<Vec<Bone>> {
    let file = PackFile::parse(data)?;
    let Some(chunk) = file.chunk(SKEL) else { return Ok(Vec::new()) };
    let mut c = chunk.cursor();
    let Some(mut skeleton_data) = c.ptr()? else { return Ok(Vec::new()) };
    let Some(mut model) = skeleton_data.ptr()? else { return Ok(Vec::new()) };
    model.string()?;
    let Some(mut skeleton) = model.ptr()? else { return Ok(Vec::new()) };
    skeleton.string()?;
    let (count, mut bone) = skeleton.array()?;
    let mut bones = Vec::with_capacity(count.min(4096));
    for _ in 0..count {
        let name = bone.string()?;
        let parent = bone.u32()? as i32;
        bone.u32()?; // transform flags
        let mut floats = [0f32; 16];
        for f in floats.iter_mut() {
            *f = bone.f32()?;
        }
        bone.skip(64 + 4)?; // inverse world matrix, LOD error
        bone.ptr()?; // extended data type
        bone.ptr()?; // extended data object
        bones.push(Bone {
            name,
            parent: usize::try_from(parent).ok().filter(|&p| p < count),
            translation: [floats[0], floats[1], floats[2]],
            rotation: [floats[3], floats[4], floats[5], floats[6]],
            scale_shear: floats[7..16].try_into().unwrap(),
        });
    }
    Ok(bones)
}

/// The external skeleton file a model is bound to.
pub fn external_skeleton(data: &[u8]) -> Result<Option<u32>> {
    let file = PackFile::parse(data)?;
    let Some(chunk) = file.chunk(SKEL) else { return Ok(None) };
    let mut c = chunk.cursor();
    c.ptr()?;
    c.file_ref()
}

/// Name token of a bone: the name after its last `:`, lower case, five bits
/// per letter (`a` = 1, first letter lowest), with a trailing number stored in
/// the top four bits. `bone:Spine02` becomes `0x2000_0000_0057_2613`.
pub fn bone_token(name: &str) -> u64 {
    let base = name.rsplit(':').next().unwrap_or(name);
    let digits = base.len() - base.trim_end_matches(|c: char| c.is_ascii_digit()).len();
    let (letters, number) = base.split_at(base.len() - digits);
    let suffix: u64 = number.parse().unwrap_or(0);
    let mut token = 0u64;
    for (i, c) in letters.chars().take(12).enumerate() {
        let value = match c.to_ascii_lowercase() {
            c @ 'a'..='z' => c as u64 - 'a' as u64 + 1,
            _ => 0,
        };
        token |= value << (5 * i);
    }
    token | (suffix & 0xf) << 60
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokens() {
        assert_eq!(bone_token("bone:Spine02"), 0x2000_0000_0057_2613);
        assert_eq!(bone_token("bone:Neck01"), 0x1000_0000_0005_8cae);
        assert_eq!(bone_token("joint"), bone_token("bone:JOINT"));
        assert_ne!(bone_token("joint"), bone_token("joint01"));
    }
}
