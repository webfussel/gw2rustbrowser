//! Sound files: `asnd` containers, PF `ASND` files and PF sound banks.

use super::pf::{self, PackFile};
use crate::error::{Error, Result};
use crate::fourcc::fcc;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Codec {
    Mp3,
    Ogg,
    Unknown(u8),
}

impl Codec {
    fn from_byte(byte: u8) -> Codec {
        match byte {
            1 => Codec::Mp3,
            2 => Codec::Ogg,
            other => Codec::Unknown(other),
        }
    }

    /// Detects the codec from the stream itself.
    fn sniff(data: &[u8]) -> Option<Codec> {
        if data.starts_with(b"OggS") {
            Some(Codec::Ogg)
        } else if data.starts_with(b"ID3") || (data.len() >= 2 && data[0] == 0xff && data[1] & 0xe0 == 0xe0) {
            Some(Codec::Mp3)
        } else {
            None
        }
    }

    pub fn extension(self) -> &'static str {
        match self {
            Codec::Mp3 => "mp3",
            Codec::Ogg => "ogg",
            Codec::Unknown(_) => "bin",
        }
    }

    pub fn mime(self) -> &'static str {
        match self {
            Codec::Mp3 => "audio/mpeg",
            Codec::Ogg => "audio/ogg",
            Codec::Unknown(_) => "application/octet-stream",
        }
    }
}

#[derive(Clone, Debug)]
pub struct Sound {
    pub codec: Codec,
    /// The playable stream (MP3 or Ogg Vorbis).
    pub data: Vec<u8>,
    pub length_seconds: Option<f32>,
}

/// Offset of the codec byte in a PF `ASND` file, for the given pointer width.
pub fn packed_codec_offset(wide: bool) -> usize {
    // file header 12 + chunk header 16 + length, offset, reserved pointer, 7 dwords
    12 + 16 + 8 + if wide { 8 } else { 4 } + 28
}

/// Extracts the stream from any single-sound file.
pub fn decode(data: &[u8]) -> Result<Sound> {
    if data.starts_with(b"asnd") {
        // Fixed 36-byte header, codec byte at offset 8.
        let stream = data.get(36..).ok_or(Error::Truncated)?.to_vec();
        let codec = Codec::sniff(&stream).unwrap_or(Codec::from_byte(data[8]));
        return Ok(Sound { codec, data: stream, length_seconds: None });
    }
    if data.starts_with(b"PF") {
        return decode_packed(data);
    }
    let codec = Codec::sniff(data).ok_or_else(|| Error::Unsupported("sound format".into()))?;
    Ok(Sound { codec, data: data.to_vec(), length_seconds: None })
}

/// PF `ASND` file: one waveform chunk.
pub fn decode_packed(data: &[u8]) -> Result<Sound> {
    let (codec, stream, length) = packed_stream(data)?;
    Ok(Sound { codec, data: stream.to_vec(), length_seconds: Some(length) })
}

/// The playable stream inside a PF `ASND` file, without copying it.
fn packed_stream(data: &[u8]) -> Result<(Codec, &[u8], f32)> {
    let file = PackFile::parse(data)?;
    let chunk = file.require_chunk(pf::ASND)?;
    if chunk.version != 2 {
        return Err(Error::Unsupported(format!("ASND chunk version {}", chunk.version)));
    }
    let mut c = chunk.cursor();
    let length = c.f32()?;
    c.f32()?; // offset
    c.ptr()?; // reserved data
    c.skip(7 * 4)?; // reserved, reserved, crc, sample count, loop start, loop end, flags
    let codec_byte = c.u8()?;
    c.skip(7)?; // reserved, channel count, reserved
    let stream = c.byte_array()?;
    // Encrypted streams (keys come from the game server) do not start with a
    // recognisable frame.
    let codec = Codec::sniff(stream).ok_or_else(|| match Codec::from_byte(codec_byte) {
        Codec::Unknown(byte) => Error::Unsupported(format!("audio codec {byte}")),
        _ => Error::Unsupported("encrypted audio".into()),
    })?;
    Ok((codec, stream, length))
}

#[derive(Debug)]
pub struct BankEntry {
    pub voice_id: u32,
    pub index: usize,
    pub sound: Result<Sound>,
}

/// Lists the sounds of a PF sound bank. Many bank entries are encrypted with
/// keys only the game server knows; those come back as errors.
pub fn decode_bank(data: &[u8]) -> Result<Vec<BankEntry>> {
    let file = PackFile::parse(data)?;
    if file.file_type != pf::ABNK {
        return Err(Error::format("sound bank", "not an ABNK file"));
    }
    let chunk = file.require_chunk(pf::BKCK)?;
    let mut c = chunk.cursor();
    c.skip(16)?;
    let (count, mut item) = c.array()?;
    let mut entries = Vec::with_capacity(count.min(4096));
    for index in 0..count {
        let voice_id = item.u32()?;
        item.skip(4 * 5 + 8 + 4)?; // flags, 4 reserved, length, offset, 4 reserved bytes
        let audio = item.byte_array()?;
        if audio.is_empty() {
            continue;
        }
        let sound = if audio.starts_with(b"PF") {
            decode_packed(audio)
        } else {
            Err(Error::Unsupported("encrypted audio".into()))
        };
        entries.push(BankEntry { voice_id, index, sound });
    }
    Ok(entries)
}

/// Voice ids of a sound bank's entries: `(voice id, entry index, playable)`.
/// Cheap: the audio streams are inspected, not copied.
pub fn bank_voices(data: &[u8]) -> Result<Vec<(u32, usize, bool)>> {
    let file = PackFile::parse(data)?;
    if file.file_type != pf::ABNK {
        return Err(Error::format("sound bank", "not an ABNK file"));
    }
    let mut c = file.require_chunk(pf::BKCK)?.cursor();
    c.skip(16)?;
    let (count, mut item) = c.array()?;
    let mut voices = Vec::with_capacity(count.min(4096));
    for index in 0..count {
        let voice_id = item.u32()?;
        item.skip(4 * 5 + 8 + 4)?;
        let audio = item.byte_array()?;
        if !audio.is_empty() {
            voices.push((voice_id, index, audio.starts_with(b"PF") && packed_stream(audio).is_ok()));
        }
    }
    Ok(voices)
}

pub const ASND_MAGIC: u32 = fcc(b"asnd");
