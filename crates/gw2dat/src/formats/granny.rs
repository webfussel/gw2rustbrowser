//! Granny 3D animation data as stored in GW2's `ANIM` chunks: the in-memory
//! structures of the Granny runtime, packed (no alignment padding), with
//! pointers stored as offsets into the data block (4 or 8 bytes wide).

use crate::error::{Error, Result};

/// Read access to one Granny data block.
#[derive(Clone, Copy)]
pub struct Blob<'a> {
    pub data: &'a [u8],
    pub wide: bool,
}

impl<'a> Blob<'a> {
    pub fn pointer_size(&self) -> usize {
        if self.wide { 8 } else { 4 }
    }

    fn bytes(&self, at: usize, n: usize) -> Result<&'a [u8]> {
        self.data.get(at..at.checked_add(n).ok_or(Error::Truncated)?).ok_or(Error::Truncated)
    }
    pub fn u8(&self, at: usize) -> Result<u8> {
        Ok(self.bytes(at, 1)?[0])
    }
    pub fn u16(&self, at: usize) -> Result<u16> {
        Ok(u16::from_le_bytes(self.bytes(at, 2)?.try_into().unwrap()))
    }
    pub fn u32(&self, at: usize) -> Result<u32> {
        Ok(u32::from_le_bytes(self.bytes(at, 4)?.try_into().unwrap()))
    }
    pub fn f32(&self, at: usize) -> Result<f32> {
        Ok(f32::from_bits(self.u32(at)?))
    }
    /// Pointer field: offset into the block, `None` for null.
    pub fn ptr(&self, at: usize) -> Result<Option<usize>> {
        let value = if self.wide {
            u64::from_le_bytes(self.bytes(at, 8)?.try_into().unwrap()) as usize
        } else {
            self.u32(at)? as usize
        };
        if value == 0 {
            return Ok(None);
        }
        if value >= self.data.len() {
            return Err(Error::format("Granny data", "pointer outside of the block"));
        }
        Ok(Some(value))
    }
    pub fn string(&self, at: usize) -> Result<String> {
        let Some(start) = self.ptr(at)? else { return Ok(String::new()) };
        let rest = &self.data[start..];
        let end = rest.iter().position(|&b| b == 0).unwrap_or(rest.len());
        Ok(String::from_utf8_lossy(&rest[..end]).into_owned())
    }
    /// Granny array `(i32 count, T* items)`; returns count and item offset.
    pub fn array(&self, at: usize) -> Result<(usize, usize)> {
        let count = self.u32(at)? as usize;
        match self.ptr(at + 4)? {
            Some(items) => Ok((count, items)),
            None if count == 0 => Ok((0, 0)),
            None => Err(Error::format("Granny data", "null array with elements")),
        }
    }
    fn f32s(&self, at: usize, n: usize) -> Result<Vec<f32>> {
        (0..n).map(|i| self.f32(at + i * 4)).collect()
    }
}

/// A decoded curve as knots (times) and controls, `dimension` values each.
#[derive(Clone, Debug, Default)]
pub struct Curve {
    pub format: u8,
    pub degree: u8,
    pub dimension: usize,
    pub knots: Vec<f32>,
    pub controls: Vec<f32>,
}

impl Curve {
    pub fn is_identity(&self) -> bool {
        self.format == FORMAT_IDENTITY
    }
}

const FORMAT_IDENTITY: u8 = 2;

pub const FORMAT_NAMES: [&str; 19] = [
    "DaKeyframes32f",
    "DaK32fC32f",
    "DaIdentity",
    "DaConstant32f",
    "D3Constant32f",
    "D4Constant32f",
    "DaK16uC16u",
    "DaK8uC8u",
    "D4nK16uC15u",
    "D4nK8uC7u",
    "D3K16uC16u",
    "D3K8uC8u",
    "D9I1K16uC16u",
    "D9I3K16uC16u",
    "D9I1K8uC8u",
    "D9I3K8uC8u",
    "D3I1K32fC32f",
    "D3I1K16uC16u",
    "D3I1K8uC8u",
];

/// Scale and offset tables of the compressed quaternion formats, selected per
/// component by the nibbles of `ScaleOffsetTableEntries`.
const QUAT_SCALES: [f32; 16] = {
    use std::f32::consts::{FRAC_1_SQRT_2 as H, SQRT_2 as S};
    [
        S,
        H,
        H / 2.0,
        H / 2.0,
        H / 2.0,
        H / 4.0,
        H / 4.0,
        H / 4.0,
        -S,
        -H,
        -H / 2.0,
        -H / 2.0,
        -H / 2.0,
        -H / 4.0,
        -H / 4.0,
        -H / 4.0,
    ]
};
const QUAT_OFFSETS: [f32; 16] = {
    use std::f32::consts::FRAC_1_SQRT_2 as H;
    [
        -H,
        -H / 2.0,
        -H * 0.75,
        -H / 4.0,
        H / 4.0,
        -H / 4.0,
        -H / 8.0,
        0.0,
        H,
        H / 2.0,
        H * 0.75,
        H / 4.0,
        -H / 4.0,
        H / 4.0,
        H / 8.0,
        -0.0,
    ]
};

/// Positions of every pointer field read while decoding, for validation
/// against the block's relocation table.
pub type PointerLog = Vec<usize>;

fn truncated_scale(bits: u16) -> f32 {
    f32::from_bits((bits as u32) << 16)
}

/// Reads `knots | controls` stored in one integer array.
fn quantized(
    blob: &Blob,
    array_at: usize,
    width: usize,
    dimension: usize,
    one_over_knot_scale: f32,
    control: impl Fn(usize, f32) -> f32,
    log: &mut PointerLog,
) -> Result<(Vec<f32>, Vec<f32>)> {
    log.push(array_at + 4);
    let (count, items) = blob.array(array_at)?;
    let knots_n = count / (dimension + 1);
    let value = |i: usize| -> Result<f32> {
        Ok(if width == 2 { blob.u16(items + i * 2)? as f32 } else { blob.u8(items + i)? as f32 })
    };
    let knots = (0..knots_n).map(|i| Ok(value(i)? / one_over_knot_scale)).collect::<Result<Vec<_>>>()?;
    let mut controls = Vec::with_capacity(knots_n * dimension);
    for i in 0..knots_n {
        for d in 0..dimension {
            controls.push(control(d, value(knots_n + i * dimension + d)?));
        }
    }
    Ok((knots, controls))
}

/// Decodes a curve object (pointed to by a `granny_curve2`).
pub fn read_curve(blob: &Blob, at: usize, log: &mut PointerLog) -> Result<Curve> {
    let p = blob.pointer_size();
    let format = blob.u8(at)?;
    let degree = blob.u8(at + 1)?;
    let mut curve = Curve { format, degree, ..Curve::default() };
    let f32_array = |array_at: usize, log: &mut PointerLog| -> Result<Vec<f32>> {
        log.push(array_at + 4);
        let (n, items) = blob.array(array_at)?;
        blob.f32s(items, n)
    };
    match format {
        0 => {
            // DaKeyframes32f: dimension, controls sampled once per knot.
            curve.dimension = blob.u16(at + 2)? as usize;
            curve.controls = f32_array(at + 4, log)?;
            let n = curve.controls.len() / curve.dimension.max(1);
            curve.knots = (0..n).map(|i| i as f32).collect();
        }
        1 => {
            curve.knots = f32_array(at + 4, log)?;
            curve.controls = f32_array(at + 8 + p, log)?;
            curve.dimension = curve.controls.len() / curve.knots.len().max(1);
        }
        FORMAT_IDENTITY => curve.dimension = blob.u16(at + 2)? as usize,
        3 => {
            curve.controls = f32_array(at + 4, log)?;
            curve.dimension = curve.controls.len();
            curve.knots = vec![0.0];
        }
        4 | 5 => {
            curve.dimension = if format == 4 { 3 } else { 4 };
            curve.controls = blob.f32s(at + 4, curve.dimension)?;
            curve.knots = vec![0.0];
        }
        6 | 7 => {
            // DaK16uC16u / DaK8uC8u: per-dimension scales then offsets.
            let scale = truncated_scale(blob.u16(at + 2)?);
            let scale_offsets = f32_array(at + 4, log)?;
            let dimension = scale_offsets.len() / 2;
            let width = if format == 6 { 2 } else { 1 };
            let (knots, controls) = quantized(
                blob,
                at + 8 + p,
                width,
                dimension,
                scale,
                |d, v| v * scale_offsets[d] + scale_offsets[dimension + d],
                log,
            )?;
            (curve.dimension, curve.knots, curve.controls) = (dimension, knots, controls);
        }
        8 | 9 => {
            // Compressed quaternions: three stored components, the largest
            // one is reconstructed; the swizzle comes from the sign bits.
            let entries = blob.u16(at + 2)?;
            let one_over_knot_scale = blob.f32(at + 4)?;
            let (bits, max) = if format == 8 { (15, 32767.0) } else { (7, 127.0) };
            let width = if format == 8 { 2 } else { 1 };
            let mut scales = [0f32; 4];
            let mut offsets = [0f32; 4];
            for i in 0..4 {
                let entry = ((entries >> (i * 4)) & 0xf) as usize;
                scales[i] = QUAT_SCALES[entry] / max;
                offsets[i] = QUAT_OFFSETS[entry];
            }
            log.push(at + 8 + 4);
            let (count, items) = blob.array(at + 8)?;
            let knots_n = count / 4;
            let raw = |i: usize| -> Result<u32> {
                Ok(if width == 2 { blob.u16(items + i * 2)? as u32 } else { blob.u8(items + i)? as u32 })
            };
            let sign = 1u32 << bits;
            let mask = sign - 1;
            curve.dimension = 4;
            for i in 0..knots_n {
                curve.knots.push(raw(i)? as f32 / one_over_knot_scale);
            }
            for i in 0..knots_n {
                let (a, b, c) = (raw(knots_n + i * 3)?, raw(knots_n + i * 3 + 1)?, raw(knots_n + i * 3 + 2)?);
                let largest = (((b & sign) != 0) as usize) << 1 | ((c & sign) != 0) as usize;
                let (s2, s3, s4) = ((largest + 1) & 3, (largest + 2) & 3, (largest + 3) & 3);
                let va = (a & mask) as f32 * scales[s2] + offsets[s2];
                let vb = (b & mask) as f32 * scales[s3] + offsets[s3];
                let vc = (c & mask) as f32 * scales[s4] + offsets[s4];
                let mut vd = (1.0 - (va * va + vb * vb + vc * vc)).max(0.0).sqrt();
                if a & sign != 0 {
                    vd = -vd;
                }
                let mut q = [0f32; 4];
                (q[s2], q[s3], q[s4], q[largest]) = (va, vb, vc, vd);
                curve.controls.extend_from_slice(&q);
            }
        }
        10 | 11 | 13 | 15 | 17 | 18 => {
            // Three scales and offsets inline. D3: vec3 controls; D9I3: the
            // diagonal of a scale matrix; D3I1: one value spread over three.
            let scale = truncated_scale(blob.u16(at + 2)?);
            let scales = blob.f32s(at + 4, 3)?;
            let offsets = blob.f32s(at + 16, 3)?;
            let width = if matches!(format, 10 | 13 | 17) { 2 } else { 1 };
            let stored = if matches!(format, 17 | 18) { 1 } else { 3 };
            let (knots, values) = quantized(blob, at + 28, width, stored, scale, |_, v| v, log)?;
            curve.dimension = 3;
            curve.knots = knots;
            for k in values.chunks_exact(stored) {
                for d in 0..3 {
                    let v = if stored == 1 { k[0] } else { k[d] };
                    curve.controls.push(v * scales[d] + offsets[d]);
                }
            }
        }
        12 | 14 => {
            // D9I1: uniform scale.
            let scale = truncated_scale(blob.u16(at + 2)?);
            let (s, o) = (blob.f32(at + 4)?, blob.f32(at + 8)?);
            let width = if format == 12 { 2 } else { 1 };
            let (knots, values) = quantized(blob, at + 12, width, 1, scale, |_, v| v * s + o, log)?;
            curve.dimension = 3;
            curve.knots = knots;
            curve.controls = values.iter().flat_map(|&v| [v, v, v]).collect();
        }
        16 => {
            // D3I1K32fC32f
            let scales = blob.f32s(at + 4, 3)?;
            let offsets = blob.f32s(at + 16, 3)?;
            log.push(at + 28 + 4);
            let (count, items) = blob.array(at + 28)?;
            let n = count / 2;
            curve.knots = blob.f32s(items, n)?;
            curve.dimension = 3;
            for i in 0..n {
                let v = blob.f32(items + (n + i) * 4)?;
                for d in 0..3 {
                    curve.controls.push(v * scales[d] + offsets[d]);
                }
            }
        }
        other => return Err(Error::Unsupported(format!("Granny curve format {other}"))),
    }
    Ok(curve)
}

#[derive(Clone, Debug)]
pub struct TransformTrack {
    pub name: String,
    pub orientation: Curve,
    pub position: Curve,
    pub scale_shear: Curve,
}

#[derive(Clone, Debug)]
pub struct Animation {
    pub name: String,
    pub duration: f32,
    pub time_step: f32,
    pub tracks: Vec<TransformTrack>,
}

/// Reads a `granny_animation` at the start of the block.
pub fn read_animation(blob: &Blob, log: &mut PointerLog) -> Result<Animation> {
    let p = blob.pointer_size();
    log.push(0);
    let name = blob.string(0)?;
    let duration = blob.f32(p)?;
    let time_step = blob.f32(p + 4)?;
    let group_count = blob.u32(p + 12)? as usize;
    log.push(p + 16);
    let mut tracks = Vec::new();
    // A few animations have no track groups at all; they are empty, not broken.
    let Some(groups) = blob.ptr(p + 16)? else { return Ok(Animation { name, duration, time_step, tracks }) };
    for g in 0..group_count {
        log.push(groups + g * p);
        let Some(group) = blob.ptr(groups + g * p)? else { continue };
        // Name*, VectorTrackCount, VectorTracks*, TransformTrackCount, TransformTracks*
        log.push(group);
        log.push(group + p + 4 + 4 + p);
        let (count, items) = blob.array(group + p + 4 + p)?;
        // transform_track: Name*, Flags, three curve2 (Type*, Object*)
        let size = p + 4 + 6 * p;
        for t in 0..count {
            let at = items + t * size;
            log.push(at);
            let name = blob.string(at)?;
            let curve = |index: usize, log: &mut PointerLog| -> Result<Curve> {
                let object_at = at + p + 4 + index * 2 * p + p;
                log.push(object_at);
                let object = blob.ptr(object_at)?.ok_or_else(|| Error::format("Granny data", "null curve"))?;
                read_curve(blob, object, log)
            };
            let orientation = curve(0, log)?;
            let position = curve(1, log)?;
            let scale_shear = curve(2, log)?;
            tracks.push(TransformTrack { name, orientation, position, scale_shear });
        }
    }
    Ok(Animation { name, duration, time_step, tracks })
}

/// Keyframes of one bone, ready for playback. Granny curves are B-splines of
/// low degree with one control per knot; using controls as keys at their
/// knot times is a close approximation at GW2's sampling rates.
#[derive(Clone, Debug, Default)]
pub struct BoneKeys {
    pub bone: String,
    pub rotation_times: Vec<f32>,
    /// Unit quaternions `x, y, z, w`.
    pub rotations: Vec<f32>,
    pub translation_times: Vec<f32>,
    pub translations: Vec<f32>,
    pub scale_times: Vec<f32>,
    /// Diagonal of the scale/shear matrix.
    pub scales: Vec<f32>,
}

fn keys(curve: &Curve, identity: &[f32]) -> (Vec<f32>, Vec<f32>) {
    if curve.is_identity() || curve.knots.is_empty() || curve.dimension == 0 {
        return (vec![0.0], identity.to_vec());
    }
    // Repeated knots (spline end conditions) would make zero-length segments.
    let d = curve.dimension;
    let (mut times, mut values) = (Vec::with_capacity(curve.knots.len()), Vec::with_capacity(curve.controls.len()));
    for (i, &t) in curve.knots.iter().enumerate() {
        let Some(control) = curve.controls.get(i * d..i * d + d) else { break };
        if times.last() == Some(&t) {
            let at = values.len() - d;
            values[at..].copy_from_slice(control);
        } else {
            times.push(t);
            values.extend_from_slice(control);
        }
    }
    if times.is_empty() {
        return (vec![0.0], identity.to_vec());
    }
    (times, values)
}

pub fn keyframes(animation: &Animation) -> Vec<BoneKeys> {
    animation
        .tracks
        .iter()
        .map(|track| {
            let (rotation_times, mut rotations) = keys(&track.orientation, &[0.0, 0.0, 0.0, 1.0]);
            for q in rotations.as_chunks_mut::<4>().0 {
                let n = (q[0] * q[0] + q[1] * q[1] + q[2] * q[2] + q[3] * q[3]).sqrt();
                if n > 0.0 {
                    q.iter_mut().for_each(|v| *v /= n);
                }
            }
            let (translation_times, translations) = keys(&track.position, &[0.0, 0.0, 0.0]);
            let (scale_times, raw_scales) = keys(&track.scale_shear, &[1.0, 1.0, 1.0]);
            // Full 3x3 matrices keep their diagonal; 3-wide curves already are.
            let scales = if track.scale_shear.dimension == 9 && raw_scales.len() % 9 == 0 {
                raw_scales.as_chunks::<9>().0.iter().flat_map(|m| [m[0], m[4], m[8]]).collect()
            } else {
                raw_scales
            };
            BoneKeys {
                bone: track.name.clone(),
                rotation_times,
                rotations,
                translation_times,
                translations,
                scale_times,
                scales,
            }
        })
        .collect()
}
