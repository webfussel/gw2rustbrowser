//! Decoding every supported image type to straight (non-premultiplied) RGBA8.

use crate::error::{Error, Result};
use crate::fourcc::{self, fcc};
use crate::inflate;

#[derive(Clone, Debug)]
pub struct RgbaImage {
    pub width: u32,
    pub height: u32,
    /// `width * height` pixels, 4 bytes each.
    pub pixels: Vec<u8>,
    /// Name of the source pixel format, for display.
    pub format: String,
}

impl RgbaImage {
    fn new(width: u32, height: u32, format: impl Into<String>) -> Self {
        RgbaImage { width, height, pixels: vec![0; width as usize * height as usize * 4], format: format.into() }
    }

    /// Encodes as PNG.
    pub fn to_png(&self) -> Result<Vec<u8>> {
        let mut out = Vec::new();
        let encoder = image::codecs::png::PngEncoder::new_with_quality(
            &mut out,
            image::codecs::png::CompressionType::Fast,
            image::codecs::png::FilterType::Adaptive,
        );
        image::ImageEncoder::write_image(
            encoder,
            &self.pixels,
            self.width,
            self.height,
            image::ExtendedColorType::Rgba8,
        )
        .map_err(|e| Error::format("PNG", e.to_string()))?;
        Ok(out)
    }
}

fn u32_at(data: &[u8], offset: usize) -> Result<u32> {
    data.get(offset..offset + 4).map(|b| u32::from_le_bytes(b.try_into().unwrap())).ok_or(Error::Truncated)
}

/// Decodes an image file (ATEX family, DDS, JPEG, PNG or WebP).
pub fn decode(data: &[u8]) -> Result<RgbaImage> {
    let magic = u32_at(data, 0)?;
    match magic {
        m if m == fcc(b"ATEX")
            || m == fcc(b"ATTX")
            || m == fcc(b"ATEC")
            || m == fcc(b"ATEP")
            || m == fcc(b"ATEU")
            || m == fcc(b"ATET")
            || m == fcc(b"CTEU") =>
        {
            decode_atex(data)
        }
        fourcc::DDS => decode_dds(data),
        _ => decode_common(data),
    }
}

fn decode_common(data: &[u8]) -> Result<RgbaImage> {
    let format = image::guess_format(data).map_err(|e| Error::format("image", e.to_string()))?;
    let decoded = image::load_from_memory_with_format(data, format)
        .map_err(|e| Error::format("image", e.to_string()))?
        .into_rgba8();
    Ok(RgbaImage {
        width: decoded.width(),
        height: decoded.height(),
        pixels: decoded.into_raw(),
        format: format!("{format:?}").to_uppercase(),
    })
}

/// Decodes an ArenaNet texture: header, then compressed DXT/BC blocks.
pub fn decode_atex(data: &[u8]) -> Result<RgbaImage> {
    let (format, width, height, blocks) = inflate::inflate_texture(data)?;
    decode_blocks(format, width as u32, height as u32, &blocks)
}

/// Decodes raw DXT/BC block data of the given FourCC format.
pub fn decode_blocks(format: u32, width: u32, height: u32, blocks: &[u8]) -> Result<RgbaImage> {
    if width == 0 || height == 0 {
        return Err(Error::format("texture", "zero size"));
    }
    let mut image = RgbaImage::new(width, height, fourcc::display(format));
    match format {
        fourcc::DXT1 => for_each_block(&mut image, blocks, 8, |b, out| decode_bc1(b, out, true))?,
        fourcc::DXT2 | fourcc::DXT3 | fourcc::DXTN => for_each_block(&mut image, blocks, 16, decode_bc2)?,
        fourcc::DXT4 | fourcc::DXT5 => for_each_block(&mut image, blocks, 16, decode_bc3)?,
        // DXT5 blocks whose colour endpoints carry a low-detail tint and whose
        // alpha channel carries the luminance; the result is opaque.
        fourcc::DXTL => {
            for_each_block(&mut image, blocks, 16, decode_bc3)?;
            for px in image.pixels.as_chunks_mut::<4>().0 {
                let luminance = px[3] as u32;
                for c in &mut px[..3] {
                    *c = (*c as u32 * luminance / 255) as u8;
                }
                px[3] = 255;
            }
        }
        fourcc::DXTA => for_each_block(&mut image, blocks, 8, |b, out| {
            let alpha = interpolate_bc4(b);
            for (i, px) in out.iter_mut().enumerate() {
                let v = alpha[i];
                *px = [v, v, v, 255];
            }
        })?,
        fourcc::_3DCX => for_each_block(&mut image, blocks, 16, decode_3dcx)?,
        fourcc::BC7X => decode_bc7(&mut image, blocks)?,
        other => return Err(Error::Unsupported(format!("texture format {}", fourcc::display(other)))),
    }
    Ok(image)
}

/// Runs `decode` for every 4x4 block and writes the result into the image,
/// clipping blocks at the right and bottom edge. Rows hold `ceil(width / 4)`
/// blocks, so textures whose size is not a multiple of four decode correctly.
fn for_each_block(
    image: &mut RgbaImage,
    blocks: &[u8],
    block_size: usize,
    decode: impl Fn(&[u8], &mut [[u8; 4]; 16]),
) -> Result<()> {
    let (width, height) = (image.width as usize, image.height as usize);
    let (blocks_x, blocks_y) = (width.div_ceil(4), height.div_ceil(4));
    if blocks.len() < blocks_x * blocks_y * block_size {
        return Err(Error::Truncated);
    }
    let mut texels = [[0u8; 4]; 16];
    for by in 0..blocks_y {
        for bx in 0..blocks_x {
            let offset = (by * blocks_x + bx) * block_size;
            decode(&blocks[offset..offset + block_size], &mut texels);
            for ty in 0..4 {
                let y = by * 4 + ty;
                if y >= height {
                    break;
                }
                for tx in 0..4 {
                    let x = bx * 4 + tx;
                    if x >= width {
                        break;
                    }
                    let at = (y * width + x) * 4;
                    image.pixels[at..at + 4].copy_from_slice(&texels[ty * 4 + tx]);
                }
            }
        }
    }
    Ok(())
}

fn rgb565(color: u16) -> [u8; 3] {
    let r = ((color >> 11) & 0x1f) as u8;
    let g = ((color >> 5) & 0x3f) as u8;
    let b = (color & 0x1f) as u8;
    [(r << 3) | (r >> 2), (g << 2) | (g >> 4), (b << 3) | (b >> 2)]
}

/// BC1 colour block. In `dxt1` mode `color0 <= color1` selects the 3-colour
/// palette with transparent black; BC2/BC3 always use four colours.
fn decode_bc1(block: &[u8], out: &mut [[u8; 4]; 16], dxt1: bool) {
    let c0 = u16::from_le_bytes([block[0], block[1]]);
    let c1 = u16::from_le_bytes([block[2], block[3]]);
    let (a, b) = (rgb565(c0), rgb565(c1));
    let mix = |wa: u16, wb: u16, div: u16| -> [u8; 4] {
        let ch = |i: usize| ((a[i] as u16 * wa + b[i] as u16 * wb) / div) as u8;
        [ch(0), ch(1), ch(2), 255]
    };
    let palette = if !dxt1 || c0 > c1 {
        [[a[0], a[1], a[2], 255], [b[0], b[1], b[2], 255], mix(2, 1, 3), mix(1, 2, 3)]
    } else {
        [[a[0], a[1], a[2], 255], [b[0], b[1], b[2], 255], mix(1, 1, 2), [0, 0, 0, 0]]
    };
    let indices = u32::from_le_bytes(block[4..8].try_into().unwrap());
    for (i, texel) in out.iter_mut().enumerate() {
        *texel = palette[((indices >> (i * 2)) & 3) as usize];
    }
}

fn decode_bc2(block: &[u8], out: &mut [[u8; 4]; 16]) {
    decode_bc1(&block[8..16], out, false);
    let alpha = u64::from_le_bytes(block[0..8].try_into().unwrap());
    for (i, texel) in out.iter_mut().enumerate() {
        let a = ((alpha >> (i * 4)) & 0xf) as u8;
        texel[3] = (a << 4) | a;
    }
}

/// Eight interpolated values plus 3-bit indices, as used by BC3 alpha, BC4 and
/// BC5. Returns the value for each of the 16 texels.
fn interpolate_bc4(block: &[u8]) -> [u8; 16] {
    let bits = u64::from_le_bytes(block[0..8].try_into().unwrap());
    let (a0, a1) = (block[0] as u32, block[1] as u32);
    let mut values = [0u8; 8];
    values[0] = a0 as u8;
    values[1] = a1 as u8;
    if a0 > a1 {
        for i in 2..8u32 {
            values[i as usize] = (((8 - i) * a0 + (i - 1) * a1) / 7) as u8;
        }
    } else {
        for i in 2..6u32 {
            values[i as usize] = (((6 - i) * a0 + (i - 1) * a1) / 5) as u8;
        }
        values[6] = 0;
        values[7] = 255;
    }
    let indices = bits >> 16;
    std::array::from_fn(|i| values[((indices >> (i * 3)) & 7) as usize])
}

fn decode_bc3(block: &[u8], out: &mut [[u8; 4]; 16]) {
    decode_bc1(&block[8..16], out, false);
    let alpha = interpolate_bc4(&block[0..8]);
    for (texel, a) in out.iter_mut().zip(alpha) {
        texel[3] = a;
    }
}

/// Two-channel normal map (BC5-like, green first). Blue is reconstructed
/// from the unit length; green is inverted to the more common convention.
fn decode_3dcx(block: &[u8], out: &mut [[u8; 4]; 16]) {
    let green = interpolate_bc4(&block[0..8]);
    let red = interpolate_bc4(&block[8..16]);
    for (i, texel) in out.iter_mut().enumerate() {
        let r = red[i] as f32 / 127.5 - 1.0;
        let g = green[i] as f32 / 127.5 - 1.0;
        let b = (1.0 - r * r - g * g).max(0.0).sqrt();
        let to_byte = |v: f32| ((v + 1.0) * 127.5) as u8;
        *texel = [to_byte(r), 255 - to_byte(g), to_byte(b), 255];
    }
}

fn decode_bc7(image: &mut RgbaImage, blocks: &[u8]) -> Result<()> {
    let (width, height) = (image.width as usize, image.height as usize);
    let (padded_w, padded_h) = (width.div_ceil(4) * 4, height.div_ceil(4) * 4);
    if blocks.len() < padded_w * padded_h {
        return Err(Error::Truncated);
    }
    let mut decoded = vec![0u32; padded_w * padded_h];
    texture2ddecoder::decode_bc7(blocks, padded_w, padded_h, &mut decoded)
        .map_err(|e| Error::format("BC7 texture", e))?;
    for y in 0..height {
        for x in 0..width {
            // texture2ddecoder yields 0xAARRGGBB.
            let [b, g, r, a] = decoded[y * padded_w + x].to_le_bytes();
            let at = (y * width + x) * 4;
            image.pixels[at..at + 4].copy_from_slice(&[r, g, b, a]);
        }
    }
    Ok(())
}

// --- DDS ----------------------------------------------------------------------

const DDS_HEADER_SIZE: usize = 128;
const DDPF_ALPHAPIXELS: u32 = 0x1;
const DDPF_FOURCC: u32 = 0x4;
const DDPF_RGB: u32 = 0x40;
const DDPF_LUMINANCE: u32 = 0x20000;
const DDPF_BUMPDUDV: u32 = 0x80000;
const D3DFMT_R32F: u32 = 114;

fn decode_dds(data: &[u8]) -> Result<RgbaImage> {
    if data.len() < DDS_HEADER_SIZE || u32_at(data, 4)? != 124 || u32_at(data, 76)? != 32 {
        return Err(Error::format("DDS", "bad header"));
    }
    let height = u32_at(data, 12)?;
    let width = u32_at(data, 16)?;
    let flags = u32_at(data, 80)?;
    let format = u32_at(data, 84)?;
    let bit_count = u32_at(data, 88)?;
    let masks = [u32_at(data, 92)?, u32_at(data, 96)?, u32_at(data, 100)?, u32_at(data, 104)?];
    let pixels = &data[DDS_HEADER_SIZE..];
    if width == 0 || height == 0 || width > 16384 || height > 16384 {
        return Err(Error::format("DDS", format!("implausible size {width}x{height}")));
    }

    let count = width as usize * height as usize;
    let mut image = RgbaImage::new(width, height, "");

    if flags & DDPF_FOURCC != 0 {
        return match format {
            fourcc::DXT1 | fourcc::DXT2 | fourcc::DXT3 | fourcc::DXT4 | fourcc::DXT5 => {
                let mut image = decode_blocks(format, width, height, pixels)?;
                image.format = format!("DDS {}", fourcc::display(format));
                Ok(image)
            }
            // D3DFMT_R32F: single-channel float (height maps), normalised to grey.
            D3DFMT_R32F => {
                image.format = "DDS R32F".into();
                let values: Vec<f32> =
                    pixels.as_chunks::<4>().0.iter().take(count).map(|&b| f32::from_le_bytes(b)).collect();
                if values.len() < count {
                    return Err(Error::Truncated);
                }
                let finite = values.iter().copied().filter(|v| v.is_finite());
                let (min, max) = finite.fold((f32::MAX, f32::MIN), |(lo, hi), v| (lo.min(v), hi.max(v)));
                let range = if max > min { max - min } else { 1.0 };
                for (v, dst) in values.iter().zip(image.pixels.as_chunks_mut::<4>().0) {
                    let l = if v.is_finite() { ((v - min) / range * 255.0) as u8 } else { 0 };
                    dst.copy_from_slice(&[l, l, l, 255]);
                }
                Ok(image)
            }
            other => Err(Error::Unsupported(format!("DDS format {}", fourcc::display(other)))),
        };
    }

    if flags & DDPF_RGB != 0 && matches!(bit_count, 16 | 24 | 32) {
        image.format = format!("DDS RGB{bit_count}");
        let bytes = bit_count as usize / 8;
        if pixels.len() < count * bytes {
            return Err(Error::Truncated);
        }
        let has_alpha = flags & DDPF_ALPHAPIXELS != 0 && masks[3] != 0;
        // Scales a masked channel of any width to 8 bits.
        let extract = |value: u32, mask: u32| -> u8 {
            if mask == 0 {
                return 0;
            }
            let raw = (value & mask) >> mask.trailing_zeros();
            let max = mask >> mask.trailing_zeros();
            (raw * 255 / max) as u8
        };
        for (src, dst) in pixels.chunks_exact(bytes).zip(image.pixels.as_chunks_mut::<4>().0) {
            let mut word = [0u8; 4];
            word[..bytes].copy_from_slice(src);
            let v = u32::from_le_bytes(word);
            let a = if has_alpha { extract(v, masks[3]) } else { 255 };
            dst.copy_from_slice(&[extract(v, masks[0]), extract(v, masks[1]), extract(v, masks[2]), a]);
        }
    } else if flags & DDPF_LUMINANCE != 0 && bit_count == 8 {
        image.format = "DDS L8".into();
        if pixels.len() < count {
            return Err(Error::Truncated);
        }
        for (&l, dst) in pixels.iter().zip(image.pixels.as_chunks_mut::<4>().0) {
            dst.copy_from_slice(&[l, l, l, 255]);
        }
    } else if flags & DDPF_BUMPDUDV != 0 && bit_count == 16 {
        // Signed two-channel bump map (V8U8), shown as a normal map.
        image.format = "DDS V8U8".into();
        if pixels.len() < count * 2 {
            return Err(Error::Truncated);
        }
        for (src, dst) in pixels.as_chunks::<2>().0.iter().zip(image.pixels.as_chunks_mut::<4>().0) {
            let (u, v) = (src[0] as i8 as f32 / 127.0, src[1] as i8 as f32 / 127.0);
            let w = (1.0 - u * u - v * v).max(0.0).sqrt();
            let to_byte = |x: f32| ((x.clamp(-1.0, 1.0) + 1.0) * 127.5) as u8;
            dst.copy_from_slice(&[to_byte(u), to_byte(v), to_byte(w), 255]);
        }
    } else {
        return Err(Error::Unsupported(format!("DDS pixel format (flags {flags:#x}, {bit_count} bpp)")));
    }
    Ok(image)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rgb565_expands_to_full_range() {
        assert_eq!(rgb565(0xffff), [255, 255, 255]);
        assert_eq!(rgb565(0xf800), [255, 0, 0]);
        assert_eq!(rgb565(0x07e0), [0, 255, 0]);
    }

    #[test]
    fn dxt1_punch_through_alpha() {
        // color0 < color1 selects the 3-colour palette; index 3 is transparent.
        let block = [0x00, 0x00, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff];
        let mut texels = [[0u8; 4]; 16];
        decode_bc1(&block, &mut texels, true);
        assert!(texels.iter().all(|t| *t == [0, 0, 0, 0]));
    }

    #[test]
    fn npot_textures_use_rounded_up_block_rows() {
        // 6x4 pixels = 2 blocks per row. Second block is red.
        let mut blocks = vec![0u8; 16];
        blocks[8..10].copy_from_slice(&0xf800u16.to_le_bytes());
        blocks[10..12].copy_from_slice(&0xf800u16.to_le_bytes());
        let image = decode_blocks(fourcc::DXT1, 6, 4, &blocks).unwrap();
        let pixel = |x: usize, y: usize| &image.pixels[(y * 6 + x) * 4..(y * 6 + x) * 4 + 4];
        assert_eq!(pixel(5, 3), &[255, 0, 0, 255]);
        assert_eq!(pixel(0, 0)[0], 0);
    }

    #[test]
    fn dxtl_multiplies_colour_by_luminance_and_is_opaque() {
        let mut block = [0u8; 16];
        block[0] = 128; // alpha endpoints: constant 128
        block[1] = 128;
        block[8..10].copy_from_slice(&0xffffu16.to_le_bytes());
        block[10..12].copy_from_slice(&0xffffu16.to_le_bytes());
        let image = decode_blocks(fourcc::DXTL, 4, 4, &block).unwrap();
        assert_eq!(&image.pixels[..4], &[128, 128, 128, 255]);
    }
}
