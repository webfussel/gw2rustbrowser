//! Star fields (PF `STAR`): the stars drawn onto the night sky.
//!
//! Each star has six values. The first two are a direction on the sky dome
//! (azimuth in radians, elevation above the horizon in radians), the fourth
//! behaves like a brightness (0..1). The meaning of the others is unknown.

use super::pf::PackFile;
use crate::error::{Error, Result};
use crate::fourcc::fcc;

pub const STAR: u32 = fcc(b"STAR");

#[derive(Clone, Debug)]
pub struct StarField {
    /// Header value of unknown meaning (0.25 or 0.125 in the known files).
    pub header: f32,
    /// `[azimuth, elevation, unknown, brightness, unknown, unknown]` per star.
    pub stars: Vec<[f32; 6]>,
}

pub fn decode(data: &[u8]) -> Result<StarField> {
    let file = PackFile::parse(data)?;
    if file.file_type != STAR {
        return Err(Error::format("star field", "not a STAR file"));
    }
    let mut c = file.require_chunk(STAR)?.cursor();
    let header = c.f32()?;
    let (count, mut item) = c.array()?;
    let mut stars = Vec::with_capacity(count.min(1 << 20));
    for _ in 0..count {
        stars.push([item.f32()?, item.f32()?, item.f32()?, item.f32()?, item.f32()?, item.f32()?]);
    }
    Ok(StarField { header, stars })
}

/// Comma-separated values with a header row.
pub fn to_csv(field: &StarField) -> String {
    let mut out = String::from("azimuth,elevation,value2,brightness,value4,value5\n");
    for s in &field.stars {
        out.push_str(&format!("{},{},{},{},{},{}\n", s[0], s[1], s[2], s[3], s[4], s[5]));
    }
    out
}
