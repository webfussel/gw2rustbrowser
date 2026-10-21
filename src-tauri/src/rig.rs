//! Skeletons, skinning and animations of models for the 3D viewer.

use std::collections::HashMap;

use gw2dat::FileType;
use gw2dat::formats::animation::{self, ClipSource};
use gw2dat::formats::granny::{self, Blob};
use gw2dat::formats::{model, skeleton};
use serde::Serialize;

use crate::session::Session;

/// The skeleton a model is animated with: its own, or its external one.
pub fn skeleton_of(session: &Session, data: &[u8]) -> Result<(Vec<skeleton::Bone>, Option<u32>), String> {
    let own = skeleton::bones(data).map_err(|e| e.to_string())?;
    let external = skeleton::external_skeleton(data).map_err(|e| e.to_string())?;
    if !own.is_empty() && (own.len() > 1 || external.is_none()) {
        return Ok((own, None));
    }
    let Some(id) = external else { return Ok((own, None)) };
    let Some(mft) = session.archive.entry_for_id(id) else { return Ok((own, None)) };
    let bones = session.archive.read(mft).ok().and_then(|d| skeleton::bones(&d).ok()).unwrap_or_default();
    Ok((bones, Some(id)))
}

/// Geometry in the file's space plus skinning as skeleton bone indices:
/// `[u32 mesh count]`, per mesh `[u32 vertices][u32 indices][u32 flags: 1 =
/// normals, 2 = uvs, 4 = skin][u32 material]`, positions, normals, uvs,
/// per vertex four skin bone indices (u16) followed by four weights (u8),
/// then indices (u32).
pub fn model_binary(session: &Session, data: &[u8]) -> Result<Vec<u8>, String> {
    let geometry = model::decode_original(data).map_err(|e| e.to_string())?;
    let (bones, _) = skeleton_of(session, data)?;
    let by_token: HashMap<u64, u16> =
        bones.iter().enumerate().map(|(i, b)| (skeleton::bone_token(&b.name), i as u16)).collect();

    let mut out = Vec::new();
    let push_u32 = |out: &mut Vec<u8>, v: u32| out.extend_from_slice(&v.to_le_bytes());
    push_u32(&mut out, geometry.meshes.len() as u32);
    for mesh in &geometry.meshes {
        let n = mesh.positions.len();
        let has_normals = mesh.normals.len() == n && n > 0;
        let has_uvs = mesh.uvs.len() == n && n > 0;
        // Skeleton bone of each binding; unknown bones fall back to the root.
        let bound: Vec<u16> = mesh.bone_bindings.iter().map(|t| by_token.get(t).copied().unwrap_or(0)).collect();
        let skinned = !bones.is_empty() && !bound.is_empty();
        push_u32(&mut out, n as u32);
        push_u32(&mut out, mesh.indices.len() as u32);
        push_u32(&mut out, has_normals as u32 | (has_uvs as u32) << 1 | (skinned as u32) << 2);
        push_u32(&mut out, mesh.material_index);
        mesh.positions.iter().flatten().for_each(|v| out.extend_from_slice(&v.to_le_bytes()));
        if has_normals {
            mesh.normals.iter().flatten().for_each(|v| out.extend_from_slice(&v.to_le_bytes()));
        }
        if has_uvs {
            mesh.uvs.iter().flatten().for_each(|v| out.extend_from_slice(&v.to_le_bytes()));
        }
        if skinned {
            let weighted = mesh.joints.len() == n && mesh.weights.len() == n;
            for v in 0..n {
                let (joints, weights) = if weighted {
                    (mesh.joints[v], mesh.weights[v])
                } else {
                    // Rigid mesh: everything follows its first bound bone.
                    ([0; 4], [255, 0, 0, 0])
                };
                for j in joints {
                    let bone = bound.get(j as usize).copied().unwrap_or(0);
                    out.extend_from_slice(&bone.to_le_bytes());
                }
                out.extend_from_slice(&weights);
            }
        }
        mesh.indices.iter().for_each(|i| out.extend_from_slice(&i.to_le_bytes()));
    }
    Ok(out)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BoneDto {
    name: String,
    parent: Option<usize>,
    translation: [f32; 3],
    rotation: [f32; 4],
    scale: [f32; 3],
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AnimationRef {
    /// File holding the animation bank.
    pub source: u32,
    pub index: usize,
    pub name: String,
    /// Where the animation comes from, for grouping in the UI.
    pub group: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RigDto {
    bones: Vec<BoneDto>,
    skeleton_file: Option<u32>,
    animations: Vec<AnimationRef>,
}

fn bank_refs(session: &Session, mft: u32, group: &str, out: &mut Vec<AnimationRef>) {
    let Ok(data) = session.archive.read(mft) else { return };
    let Ok(entries) = animation::bank(&data) else { return };
    for entry in entries {
        // The rest pose is not an animation.
        if entry.name == "zeropose" || matches!(entry.source, ClipSource::Missing) {
            continue;
        }
        out.push(AnimationRef { source: mft, index: entry.index, name: entry.name, group: group.to_string() });
    }
}

/// Skeleton and every animation that fits it: the model's own animation bank
/// and all animation sets made for its skeleton.
pub fn rig(session: &Session, mft: u32) -> Result<RigDto, String> {
    let data = session.read(mft).map_err(|e| e.to_string())?;
    let (bones, skeleton_file) = skeleton_of(session, &data)?;
    let mut animations = Vec::new();
    // Same sources as the "Models with animation" category (see gw2dat::index).
    if !bones.is_empty() {
        bank_refs(session, mft, "This model", &mut animations);
        if let Some(skeleton_mft) = skeleton_file.and_then(|id| session.archive.entry_for_id(id)) {
            bank_refs(session, skeleton_mft, "Skeleton", &mut animations);
        }
        let skeleton_id = skeleton_file.or_else(|| session.entry(mft).map(|e| e.base_id));
        if let Some(skeleton_id) = skeleton_id {
            let sets = session
                .index
                .entries
                .iter()
                .filter(|e| e.file_type == FileType::ModelAnimationSet && e.reference == skeleton_id);
            for set in sets {
                bank_refs(session, set.mft_index, &format!("Animation set {}", set.base_id), &mut animations);
            }
        }
    }
    let bones = bones
        .into_iter()
        .map(|b| BoneDto {
            name: b.name,
            parent: b.parent,
            translation: b.translation,
            rotation: b.rotation,
            scale: [b.scale_shear[0], b.scale_shear[4], b.scale_shear[8]],
        })
        .collect();
    Ok(RigDto { bones, skeleton_file, animations })
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TrackDto {
    bone: String,
    rotation_times: Vec<f32>,
    rotations: Vec<f32>,
    translation_times: Vec<f32>,
    translations: Vec<f32>,
    scale_times: Vec<f32>,
    scales: Vec<f32>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClipDto {
    name: String,
    duration: f32,
    tracks: Vec<TrackDto>,
}

/// Decodes one bank entry into keyframes.
pub fn clip(session: &Session, source: u32, index: usize) -> Result<ClipDto, String> {
    let bank_data = session.archive.read(source).map_err(|e| e.to_string())?;
    let entries = animation::bank(&bank_data).map_err(|e| e.to_string())?;
    let entry = entries.get(index).ok_or("No such animation")?;
    let external = match entry.source {
        ClipSource::External(id) => session.archive.entry_for_id(id).and_then(|mft| session.archive.read(mft).ok()),
        _ => None,
    };
    let data = match &external {
        Some(clip_file) => animation::clip_file_data(clip_file),
        None => animation::bank_clip_data(&bank_data, index),
    }
    .map_err(|e| e.to_string())?;
    let blob = Blob { data: data.block, wide: data.wide };
    let parsed = granny::read_animation(&blob, &mut Vec::new()).map_err(|e| e.to_string())?;
    let tracks = granny::keyframes(&parsed)
        .into_iter()
        .map(|k| TrackDto {
            bone: k.bone,
            rotation_times: k.rotation_times,
            rotations: k.rotations,
            translation_times: k.translation_times,
            translations: k.translations,
            scale_times: k.scale_times,
            scales: k.scales,
        })
        .collect();
    Ok(ClipDto { name: entry.name.clone(), duration: parsed.duration, tracks })
}
