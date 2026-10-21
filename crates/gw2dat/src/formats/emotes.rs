//! Emote animations (PF `emoc`, chunk `anim`): how each emote plays.
//!
//! The archive has one such file. Each emote (a name token like `wave` or
//! `eata`) lists timings, usually one per body type or mount: the model or
//! animation set holding the animation, blend times and the lengths of the
//! intro, loop and outro parts.

use super::pf::PackFile;
use crate::error::{Error, Result};
use crate::fourcc::fcc;

pub const EMOC: u32 = fcc(b"emoc");
const ANIM: u32 = fcc(b"anim");

#[derive(Clone, Debug)]
pub struct Emote {
    pub name: u64,
    pub timings: Vec<Timing>,
}

/// `PackEmoteTimingV3`.
#[derive(Clone, Debug)]
pub struct Timing {
    pub model: Option<u32>,
    /// Blend times in seconds.
    pub blend_in: f32,
    pub blend_out: f32,
    /// Part lengths and start offset in milliseconds.
    pub intro: u32,
    pub loop_length: u32,
    pub outro: u32,
    pub start_offset: u32,
}

pub fn decode(data: &[u8]) -> Result<Vec<Emote>> {
    let file = PackFile::parse(data)?;
    if file.file_type != EMOC {
        return Err(Error::format("emote animations", "not an emoc file"));
    }
    let chunk = file.require_chunk(ANIM)?;
    if chunk.version != 3 {
        return Err(Error::Unsupported(format!("emote chunk version {}", chunk.version)));
    }
    let mut c = chunk.cursor();
    let (count, mut item) = c.array()?;
    let mut emotes = Vec::with_capacity(count.min(1 << 16));
    for _ in 0..count {
        let name = item.u64()?;
        let (timings, mut t) = item.array()?;
        let timings = (0..timings)
            .map(|_| {
                Ok(Timing {
                    model: t.file_ref()?,
                    blend_in: t.f32()?,
                    blend_out: t.f32()?,
                    intro: t.u32()?,
                    loop_length: t.u32()?,
                    outro: t.u32()?,
                    start_offset: t.u32()?,
                })
            })
            .collect::<Result<_>>()?;
        emotes.push(Emote { name, timings });
    }
    Ok(emotes)
}

/// One row per timing, with a header.
pub fn to_csv(emotes: &[Emote]) -> String {
    let mut out = String::from("emote,model,blend_in,blend_out,intro_ms,loop_ms,outro_ms,start_offset_ms\n");
    for e in emotes {
        let name = super::material::token_label(e.name);
        for t in &e.timings {
            out.push_str(&format!(
                "{name},{},{},{},{},{},{},{}\n",
                t.model.map_or(String::new(), |m| m.to_string()),
                t.blend_in,
                t.blend_out,
                t.intro,
                t.loop_length,
                t.outro,
                t.start_offset
            ));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A 64-bit emoc file with the emote `go` and one timing for file 6.
    fn file() -> Vec<u8> {
        let mut p = vec![0u8; 12 + 8 + 12 + 40 + 6];
        p[0] = 1; // one emote, elements follow the 12-byte array header
        p[4] = 8;
        p[12..20].copy_from_slice(&0x1e7u64.to_le_bytes()); // "go"
        p[20] = 1; // one timing
        p[24] = 8;
        let t = 32;
        p[t] = 40; // file reference after the timing
        p[t + 8..t + 12].copy_from_slice(&0.25f32.to_le_bytes());
        p[t + 16..t + 20].copy_from_slice(&3666u32.to_le_bytes());
        p[t + 40..t + 46].copy_from_slice(&[0x05, 0x01, 0x00, 0x01, 0, 0]);
        let mut data = b"PF\x05\x00\x00\x00\x0c\x00emocanim".to_vec();
        data.extend_from_slice(&((p.len() + 8) as u32).to_le_bytes());
        data.extend_from_slice(&[3, 0, 16, 0, 0, 0, 0, 0]);
        data.extend_from_slice(&p);
        data
    }

    #[test]
    fn decodes_timings() {
        let emotes = decode(&file()).unwrap();
        assert_eq!(emotes.len(), 1);
        assert_eq!(super::super::material::token_name(emotes[0].name).as_deref(), Some("go"));
        let t = &emotes[0].timings[0];
        assert_eq!((t.model, t.blend_in, t.intro), (Some(6), 0.25, 3666));
        assert_eq!(
            to_csv(&emotes),
            "emote,model,blend_in,blend_out,intro_ms,loop_ms,outro_ms,start_offset_ms\ngo,6,0.25,0,3666,0,0,0\n"
        );
    }
}
