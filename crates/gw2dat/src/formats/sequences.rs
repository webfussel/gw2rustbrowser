//! Animation sequences (PF `anic`, chunk `seqn`): how game actions play
//! their animations — steps with durations, optional movement, blend times,
//! timed triggers and charge stages. Names are animation tokens.

use super::animation::token_name;
use super::pf::{Cursor, PackFile};
use crate::error::{Error, Result};
use crate::fourcc::fcc;

pub const ANIC: u32 = fcc(b"anic");
pub const SEQN: u32 = fcc(b"seqn");

#[derive(Clone, Debug)]
pub struct Movement {
    /// Direction of travel and facing, as quaternions `x, y, z, w`.
    pub move_rotation: [f32; 4],
    pub facing_rotation: [f32; 4],
    pub velocity: [f32; 2],
}

#[derive(Clone, Debug)]
pub struct Step {
    /// 0 = action, 1 = movement.
    pub kind: u8,
    pub animation_speed: f32,
    pub flags: u32,
    /// Milliseconds.
    pub duration: u32,
    pub movement: Option<Movement>,
}

#[derive(Clone, Debug)]
pub struct Trigger {
    /// Kind of event; the meaning of the values is unknown.
    pub kind: u8,
    /// Milliseconds from the start of the variant.
    pub time: u32,
    pub flags: u32,
}

#[derive(Clone, Debug)]
pub struct ChargeStage {
    pub duration: u16,
    pub level: u8,
}

#[derive(Clone, Debug)]
pub struct Variant {
    pub name: String,
    pub flags: u32,
    pub blend_in: f32,
    pub blend_out: f32,
    pub charge_stages: Vec<ChargeStage>,
    pub steps: Vec<Step>,
    pub triggers: Vec<Trigger>,
}

impl Variant {
    /// Sum of the step durations in milliseconds.
    pub fn duration(&self) -> u32 {
        self.steps.iter().map(|s| s.duration).sum()
    }
}

#[derive(Clone, Debug)]
pub struct Sequence {
    pub name: String,
    pub variants: Vec<Variant>,
}

fn quaternion(c: &mut Cursor) -> Result<[f32; 4]> {
    Ok([c.f32()?, c.f32()?, c.f32()?, c.f32()?])
}

fn read_variant(c: &mut Cursor) -> Result<Variant> {
    let name = token_name(c.u64()?);
    let flags = c.u32()?;
    let (blend_in, blend_out) = (c.f32()?, c.f32()?);
    let (count, mut item) = c.array()?;
    let mut charge_stages = Vec::with_capacity(count);
    for _ in 0..count {
        charge_stages.push(ChargeStage { duration: item.u16()?, level: item.u8()? });
    }
    let (count, mut item) = c.array()?;
    let mut steps = Vec::with_capacity(count);
    for _ in 0..count {
        let kind = item.u8()?;
        let animation_speed = item.f32()?;
        let flags = item.u32()?;
        let action = item.ptr()?;
        let movement = item.ptr()?;
        let mut duration = 0;
        if let Some(mut action) = action {
            duration = action.u32()?;
        }
        let movement = match movement {
            Some(mut m) => {
                duration = m.u32()?;
                Some(Movement {
                    move_rotation: quaternion(&mut m)?,
                    facing_rotation: quaternion(&mut m)?,
                    velocity: [m.f32()?, m.f32()?],
                })
            }
            None => None,
        };
        steps.push(Step { kind, animation_speed, flags, duration, movement });
    }
    let (count, mut item) = c.array()?;
    let mut triggers = Vec::with_capacity(count);
    for _ in 0..count {
        triggers.push(Trigger { kind: item.u8()?, time: item.u32()?, flags: item.u32()? });
    }
    Ok(Variant { name, flags, blend_in, blend_out, charge_stages, steps, triggers })
}

pub fn decode(data: &[u8]) -> Result<Vec<Sequence>> {
    let file = PackFile::parse(data)?;
    if file.file_type != ANIC {
        return Err(Error::format("animation sequences", "not an anic file"));
    }
    let (count, mut item) = file.require_chunk(SEQN)?.cursor().array()?;
    let mut sequences = Vec::with_capacity(count.min(1 << 16));
    for _ in 0..count {
        let name = token_name(item.u64()?);
        let (variants_count, mut variant) = item.array()?;
        let mut variants = Vec::with_capacity(variants_count.min(1 << 12));
        for _ in 0..variants_count {
            variants.push(read_variant(&mut variant)?);
        }
        sequences.push(Sequence { name, variants });
    }
    Ok(sequences)
}

fn json_string(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 2);
    out.push('"');
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// The whole table as JSON (for export).
pub fn to_json(sequences: &[Sequence]) -> String {
    use std::fmt::Write;
    let floats = |v: &[f32]| v.iter().map(|f| f.to_string()).collect::<Vec<_>>().join(",");
    let comma = |i: usize| if i > 0 { "," } else { "" };
    let mut out = String::from("[\n");
    for (i, s) in sequences.iter().enumerate() {
        let _ = write!(out, "  {{\"name\":{},\"variants\":[", json_string(&s.name));
        for (j, v) in s.variants.iter().enumerate() {
            let _ = write!(
                out,
                "{}{{\"name\":{},\"flags\":{},\"blendIn\":{},\"blendOut\":{},\"steps\":[",
                comma(j),
                json_string(&v.name),
                v.flags,
                v.blend_in,
                v.blend_out
            );
            for (k, st) in v.steps.iter().enumerate() {
                let _ = write!(
                    out,
                    "{}{{\"kind\":{},\"durationMs\":{},\"animationSpeed\":{},\"flags\":{}",
                    comma(k),
                    st.kind,
                    st.duration,
                    st.animation_speed,
                    st.flags
                );
                if let Some(m) = &st.movement {
                    let _ = write!(
                        out,
                        ",\"moveRotation\":[{}],\"facingRotation\":[{}],\"velocity\":[{}]",
                        floats(&m.move_rotation),
                        floats(&m.facing_rotation),
                        floats(&m.velocity)
                    );
                }
                out.push('}');
            }
            out.push_str("],\"triggers\":[");
            for (k, t) in v.triggers.iter().enumerate() {
                let _ = write!(out, "{}{{\"kind\":{},\"timeMs\":{},\"flags\":{}}}", comma(k), t.kind, t.time, t.flags);
            }
            out.push_str("],\"chargeStages\":[");
            for (k, c) in v.charge_stages.iter().enumerate() {
                let _ = write!(out, "{}{{\"duration\":{},\"level\":{}}}", comma(k), c.duration, c.level);
            }
            out.push_str("]}");
        }
        let _ = writeln!(out, "]}}{}", if i + 1 < sequences.len() { "," } else { "" });
    }
    out.push_str("]\n");
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_escaping() {
        assert_eq!(json_string("a\"b\\c"), r#""a\"b\\c""#);
    }
}
