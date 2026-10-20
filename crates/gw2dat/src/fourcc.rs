//! Four-character codes, as little-endian `u32` values read from the data.

pub const fn fcc(code: &[u8; 4]) -> u32 {
    u32::from_le_bytes(*code)
}

/// Renders a FourCC for display, replacing non-printable bytes with `.`.
pub fn display(value: u32) -> String {
    value.to_le_bytes().iter().map(|&b| if b.is_ascii_graphic() || b == b' ' { b as char } else { '.' }).collect()
}

// Texture block formats.
pub const DXT1: u32 = fcc(b"DXT1");
pub const DXT2: u32 = fcc(b"DXT2");
pub const DXT3: u32 = fcc(b"DXT3");
pub const DXT4: u32 = fcc(b"DXT4");
pub const DXT5: u32 = fcc(b"DXT5");
pub const DXTA: u32 = fcc(b"DXTA");
pub const DXTL: u32 = fcc(b"DXTL");
pub const DXTN: u32 = fcc(b"DXTN");
pub const _3DCX: u32 = fcc(b"3DCX");
pub const BC7X: u32 = fcc(b"BC7X");

pub const DDS: u32 = fcc(b"DDS ");
pub const RIFF: u32 = fcc(b"RIFF");
pub const WEBP: u32 = fcc(b"WEBP");
