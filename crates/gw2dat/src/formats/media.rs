//! Headers of formats that are only described, not decoded: Bink 2 videos
//! (RAD Game Tools' proprietary codec) and Flash movies.

use std::io::Read;

use crate::error::{Error, Result};

fn u32_at(data: &[u8], at: usize) -> Result<u32> {
    data.get(at..at + 4).map(|b| u32::from_le_bytes(b.try_into().unwrap())).ok_or(Error::Truncated)
}

#[derive(Clone, Debug)]
pub struct BinkInfo {
    /// Format revision letter after `KB2`.
    pub revision: char,
    pub frames: u32,
    pub largest_frame: u32,
    pub width: u32,
    pub height: u32,
    pub fps_numerator: u32,
    pub fps_denominator: u32,
    pub flags: u32,
    pub audio_tracks: u32,
}

impl BinkInfo {
    pub fn fps(&self) -> f64 {
        if self.fps_denominator == 0 { 0.0 } else { self.fps_numerator as f64 / self.fps_denominator as f64 }
    }

    pub fn duration(&self) -> f64 {
        let fps = self.fps();
        if fps > 0.0 { self.frames as f64 / fps } else { 0.0 }
    }
}

/// Header of a Bink 2 file (`KB2` + revision letter). Same layout as Bink 1:
/// size, frames, largest frame, (frames), width, height, fps, flags, audio tracks.
pub fn bink(data: &[u8]) -> Result<BinkInfo> {
    if !data.starts_with(b"KB2") || data.len() < 44 {
        return Err(Error::format("Bink video", "not a Bink 2 file"));
    }
    Ok(BinkInfo {
        revision: data[3] as char,
        frames: u32_at(data, 8)?,
        largest_frame: u32_at(data, 12)?,
        width: u32_at(data, 20)?,
        height: u32_at(data, 24)?,
        fps_numerator: u32_at(data, 28)?,
        fps_denominator: u32_at(data, 32)?,
        flags: u32_at(data, 36)?,
        audio_tracks: u32_at(data, 40)?,
    })
}

#[derive(Clone, Debug)]
pub struct FlashInfo {
    /// `zlib`, `LZMA` or `none`.
    pub compression: &'static str,
    pub version: u8,
    pub length: u32,
    /// Stage size in pixels (when the header could be read).
    pub width: Option<f32>,
    pub height: Option<f32>,
    pub frame_rate: Option<f32>,
    pub frames: Option<u16>,
}

/// Header of a Flash movie (`FWS`, zlib-compressed `CWS` or LZMA `ZWS`).
pub fn flash(data: &[u8]) -> Result<FlashInfo> {
    let compression = match data.get(..3) {
        Some(b"FWS") => "none",
        Some(b"CWS") => "zlib",
        Some(b"ZWS") => "LZMA",
        _ => return Err(Error::format("Flash", "not a SWF file")),
    };
    let version = *data.get(3).ok_or(Error::Truncated)?;
    let length = u32_at(data, 4)?;
    let body: Option<Vec<u8>> = match compression {
        "none" => Some(data[8..].to_vec()),
        "zlib" => {
            let mut out = Vec::new();
            flate2::read::ZlibDecoder::new(&data[8..]).take(64).read_to_end(&mut out).ok().map(|_| out)
        }
        _ => None,
    };
    let mut info =
        FlashInfo { compression, version, length, width: None, height: None, frame_rate: None, frames: None };
    if let Some(body) = body {
        // Stage RECT: 5 bits field width, then xmin, xmax, ymin, ymax in twips.
        let bit = |i: usize| body.get(i / 8).map(|b| (b >> (7 - i % 8)) & 1);
        let bits = |from: usize, n: usize| -> Option<i32> {
            let mut v: u32 = 0;
            for i in 0..n {
                v = v << 1 | bit(from + i)? as u32;
            }
            // Sign-extend.
            Some(if n > 0 && v >> (n - 1) & 1 == 1 { (v | !0u32 << n) as i32 } else { v as i32 })
        };
        if let Some(n) = bits(0, 5).map(|n| (n as u32 & 31) as usize) {
            let values: Option<Vec<i32>> = (0..4).map(|i| bits(5 + i * n, n)).collect();
            if let Some(v) = values {
                info.width = Some((v[1] - v[0]) as f32 / 20.0);
                info.height = Some((v[3] - v[2]) as f32 / 20.0);
                let at = (5 + 4 * n).div_ceil(8);
                if let Some(b) = body.get(at..at + 4) {
                    info.frame_rate = Some(b[1] as f32 + b[0] as f32 / 256.0);
                    info.frames = Some(u16::from_le_bytes([b[2], b[3]]));
                }
            }
        }
    }
    Ok(info)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bink_header() {
        let mut data = b"KB2i".to_vec();
        for v in [1000u32, 23, 5000, 23, 768, 144, 30, 1, 0, 2] {
            data.extend_from_slice(&v.to_le_bytes());
        }
        let b = bink(&data).unwrap();
        assert_eq!((b.revision, b.frames, b.width, b.height, b.audio_tracks), ('i', 23, 768, 144, 2));
        assert!((b.duration() - 23.0 / 30.0).abs() < 1e-9);
    }

    #[test]
    fn flash_header() {
        // FWS v10, RECT with 15-bit fields: 0, 11000 (550 px), 0, 8000 (400 px); 24 fps, 1 frame.
        let mut body = vec![0u8; 0];
        let mut bits: Vec<u8> = Vec::new();
        let mut push = |v: u32, n: usize| (0..n).rev().for_each(|i| bits.push((v >> i & 1) as u8));
        push(15, 5);
        for v in [0, 11000, 0, 8000] {
            push(v, 15);
        }
        while !bits.len().is_multiple_of(8) {
            bits.push(0);
        }
        body.extend(bits.chunks(8).map(|c| c.iter().fold(0, |a, &b| a << 1 | b)));
        body.extend_from_slice(&[0, 24, 1, 0]);
        let mut data = b"FWS\x0a".to_vec();
        data.extend_from_slice(&((8 + body.len()) as u32).to_le_bytes());
        data.extend_from_slice(&body);
        let f = flash(&data).unwrap();
        assert_eq!(
            (f.version, f.width, f.height, f.frame_rate, f.frames),
            (10, Some(550.0), Some(400.0), Some(24.0), Some(1))
        );
    }
}
