//! Decompression of compressed MFT entries (ArenaNet's Huffman + LZ77 scheme).
//! Port of gw2DatTools' `inflateDatFileBuffer`.

use std::sync::OnceLock;

use super::bits::{BitReader, BitSource};
use super::huffman::{Code, HuffmanBuilder, HuffmanTree, MAX_SYMBOLS};
use crate::error::{Error, Result};

/// Every 64 KiB block of a compressed entry ends with a 4-byte checksum.
const BLOCK_WORDS: usize = 0x4000;

/// Fixed tree used to decode the code lengths of the per-section trees.
/// Order matters, see [`HuffmanBuilder::add`].
const DICTIONARY: [(u16, u8); 256] = [
    (0x0A, 3),
    (0x09, 3),
    (0x08, 3),
    (0x0C, 4),
    (0x0B, 4),
    (0x07, 4),
    (0x00, 4),
    (0xE0, 5),
    (0x2A, 5),
    (0x29, 5),
    (0x06, 5),
    (0x4A, 6),
    (0x40, 6),
    (0x2C, 6),
    (0x2B, 6),
    (0x28, 6),
    (0x20, 6),
    (0x05, 6),
    (0x04, 6),
    (0x49, 7),
    (0x48, 7),
    (0x27, 7),
    (0x26, 7),
    (0x25, 7),
    (0x0D, 7),
    (0x03, 7),
    (0x6A, 8),
    (0x69, 8),
    (0x4C, 8),
    (0x4B, 8),
    (0x47, 8),
    (0x24, 8),
    (0xE8, 9),
    (0xA0, 9),
    (0x89, 9),
    (0x88, 9),
    (0x68, 9),
    (0x67, 9),
    (0x63, 9),
    (0x60, 9),
    (0x46, 9),
    (0x23, 9),
    (0xE9, 10),
    (0xC9, 10),
    (0xC0, 10),
    (0xA9, 10),
    (0xA8, 10),
    (0x8A, 10),
    (0x87, 10),
    (0x80, 10),
    (0x66, 10),
    (0x65, 10),
    (0x45, 10),
    (0x44, 10),
    (0x43, 10),
    (0x2D, 10),
    (0x02, 10),
    (0x01, 10),
    (0xE5, 11),
    (0xC8, 11),
    (0xAA, 11),
    (0xA5, 11),
    (0xA4, 11),
    (0x8B, 11),
    (0x85, 11),
    (0x84, 11),
    (0x6C, 11),
    (0x6B, 11),
    (0x64, 11),
    (0x4D, 11),
    (0x0E, 11),
    (0xE7, 12),
    (0xCA, 12),
    (0xC7, 12),
    (0xA7, 12),
    (0xA6, 12),
    (0x86, 12),
    (0x83, 12),
    (0xE6, 13),
    (0xE4, 13),
    (0xC4, 13),
    (0x8C, 13),
    (0x2E, 13),
    (0x22, 13),
    (0xEC, 14),
    (0xC6, 14),
    (0x6D, 14),
    (0x4E, 14),
    (0xEA, 15),
    (0xCC, 15),
    (0xAC, 15),
    (0xAB, 15),
    (0x8D, 15),
    (0x11, 15),
    (0x10, 15),
    (0x0F, 15),
    (0xFF, 16),
    (0xFE, 16),
    (0xFD, 16),
    (0xFC, 16),
    (0xFB, 16),
    (0xFA, 16),
    (0xF9, 16),
    (0xF8, 16),
    (0xF7, 16),
    (0xF6, 16),
    (0xF5, 16),
    (0xF4, 16),
    (0xF3, 16),
    (0xF2, 16),
    (0xF1, 16),
    (0xF0, 16),
    (0xEF, 16),
    (0xEE, 16),
    (0xED, 16),
    (0xEB, 16),
    (0xE3, 16),
    (0xE2, 16),
    (0xE1, 16),
    (0xDF, 16),
    (0xDE, 16),
    (0xDD, 16),
    (0xDC, 16),
    (0xDB, 16),
    (0xDA, 16),
    (0xD9, 16),
    (0xD8, 16),
    (0xD7, 16),
    (0xD6, 16),
    (0xD5, 16),
    (0xD4, 16),
    (0xD3, 16),
    (0xD2, 16),
    (0xD1, 16),
    (0xD0, 16),
    (0xCF, 16),
    (0xCE, 16),
    (0xCD, 16),
    (0xCB, 16),
    (0xC5, 16),
    (0xC3, 16),
    (0xC2, 16),
    (0xC1, 16),
    (0xBF, 16),
    (0xBE, 16),
    (0xBD, 16),
    (0xBC, 16),
    (0xBB, 16),
    (0xBA, 16),
    (0xB9, 16),
    (0xB8, 16),
    (0xB7, 16),
    (0xB6, 16),
    (0xB5, 16),
    (0xB4, 16),
    (0xB3, 16),
    (0xB2, 16),
    (0xB1, 16),
    (0xB0, 16),
    (0xAF, 16),
    (0xAE, 16),
    (0xAD, 16),
    (0xA3, 16),
    (0xA2, 16),
    (0xA1, 16),
    (0x9F, 16),
    (0x9E, 16),
    (0x9D, 16),
    (0x9C, 16),
    (0x9B, 16),
    (0x9A, 16),
    (0x99, 16),
    (0x98, 16),
    (0x97, 16),
    (0x96, 16),
    (0x95, 16),
    (0x94, 16),
    (0x93, 16),
    (0x92, 16),
    (0x91, 16),
    (0x90, 16),
    (0x8F, 16),
    (0x8E, 16),
    (0x82, 16),
    (0x81, 16),
    (0x7F, 16),
    (0x7E, 16),
    (0x7D, 16),
    (0x7C, 16),
    (0x7B, 16),
    (0x7A, 16),
    (0x79, 16),
    (0x78, 16),
    (0x77, 16),
    (0x76, 16),
    (0x75, 16),
    (0x74, 16),
    (0x73, 16),
    (0x72, 16),
    (0x71, 16),
    (0x70, 16),
    (0x6F, 16),
    (0x6E, 16),
    (0x62, 16),
    (0x61, 16),
    (0x5F, 16),
    (0x5E, 16),
    (0x5D, 16),
    (0x5C, 16),
    (0x5B, 16),
    (0x5A, 16),
    (0x59, 16),
    (0x58, 16),
    (0x57, 16),
    (0x56, 16),
    (0x55, 16),
    (0x54, 16),
    (0x53, 16),
    (0x52, 16),
    (0x51, 16),
    (0x50, 16),
    (0x4F, 16),
    (0x42, 16),
    (0x41, 16),
    (0x3F, 16),
    (0x3E, 16),
    (0x3D, 16),
    (0x3C, 16),
    (0x3B, 16),
    (0x3A, 16),
    (0x39, 16),
    (0x38, 16),
    (0x37, 16),
    (0x36, 16),
    (0x35, 16),
    (0x34, 16),
    (0x33, 16),
    (0x32, 16),
    (0x31, 16),
    (0x30, 16),
    (0x2F, 16),
    (0x21, 16),
    (0x1F, 16),
    (0x1E, 16),
    (0x1D, 16),
    (0x1C, 16),
    (0x1B, 16),
    (0x1A, 16),
    (0x19, 16),
    (0x18, 16),
    (0x17, 16),
    (0x16, 16),
    (0x15, 16),
    (0x14, 16),
    (0x13, 16),
    (0x12, 16),
];

fn dictionary() -> &'static HuffmanTree {
    static TREE: OnceLock<HuffmanTree> = OnceLock::new();
    TREE.get_or_init(|| {
        let mut builder = HuffmanBuilder::new();
        for &(symbol, bits) in &DICTIONARY {
            builder.add(symbol, bits).expect("static dictionary is valid");
        }
        builder.build().expect("static dictionary is valid").expect("static dictionary is not empty")
    })
}

/// Uncompressed size as stored in the header of a compressed entry.
pub fn inflated_size(input: &[u8]) -> Result<u32> {
    let bytes = input.get(4..8).ok_or(Error::Truncated)?;
    Ok(u32::from_le_bytes(bytes.try_into().unwrap()))
}

/// Inflates a compressed entry. At most `limit` bytes are produced, which makes
/// peeking at the start of a file cheap: decoding stops as soon as enough
/// output exists, and only the input consumed so far needs to be present.
pub fn inflate(input: &[u8], limit: Option<u32>) -> Result<Vec<u8>> {
    let mut bits = BitReader::new(input, BLOCK_WORDS);
    bits.take(32)?;
    let mut size = bits.take(32)?;
    if let Some(limit) = limit {
        size = size.min(limit);
    }
    let mut output = Vec::with_capacity(size as usize);
    inflate_into(&mut bits, size as usize, &mut output)?;
    Ok(output)
}

fn read_code_lengths(bits: &mut BitReader, builder: &mut HuffmanBuilder) -> Result<Code> {
    let symbol_count = bits.take(16)? as i32;
    if symbol_count as usize > MAX_SYMBOLS {
        return Err(Error::Decompress("too many huffman symbols"));
    }
    builder.clear();
    let dictionary = dictionary();
    let mut remaining = symbol_count - 1;
    let single = remaining as u16;

    while remaining >= 0 {
        let code = dictionary.read(bits)?;
        let code_bits = (code & 0x1f) as u8;
        let mut count = ((code >> 5) + 1) as i32;
        if code_bits == 0 {
            remaining -= count;
            continue;
        }
        while count > 0 {
            if remaining < 0 {
                return Err(Error::Decompress("huffman code lengths overflow"));
            }
            builder.add(remaining as u16, code_bits)?;
            remaining -= 1;
            count -= 1;
        }
    }

    Ok(match builder.build()? {
        Some(tree) => Code::Tree(Box::new(tree)),
        None => Code::Single(single),
    })
}

fn inflate_into(bits: &mut BitReader, size: usize, output: &mut Vec<u8>) -> Result<()> {
    bits.take(4)?;
    let copy_size_bias = bits.take(4)? + 1;
    let mut builder = HuffmanBuilder::new();

    while output.len() < size {
        let symbols = read_code_lengths(bits, &mut builder)?;
        let offsets = read_code_lengths(bits, &mut builder)?;
        let max_codes = (bits.take(4)? + 1) << 12;

        let mut codes = 0;
        while codes < max_codes && output.len() < size {
            codes += 1;
            let symbol = symbols.read(bits)?;
            if symbol < 0x100 {
                output.push(symbol as u8);
                continue;
            }

            // Back-reference: length code, then distance code, each followed
            // by a number of extra bits.
            let symbol = (symbol - 0x100) as u32;
            let (quot, rem) = (symbol / 4, symbol % 4);
            let mut copy_size = match quot {
                0 => symbol,
                1..=6 => (1 << (quot - 1)) * (4 + rem),
                _ if symbol == 28 => 0xff,
                _ => return Err(Error::Decompress("invalid copy size code")),
            };
            if quot > 1 && symbol != 28 {
                copy_size |= bits.take(quot - 1)?;
            }
            copy_size += copy_size_bias;

            let symbol = offsets.read(bits)? as u32;
            let (quot, rem) = (symbol / 2, symbol % 2);
            let mut distance = match quot {
                0 => symbol,
                1..=16 => (1 << (quot - 1)) * (2 + rem),
                _ => return Err(Error::Decompress("invalid copy offset code")),
            };
            if quot > 1 {
                distance |= bits.take(quot - 1)?;
            }
            let distance = distance as usize + 1;

            if distance > output.len() {
                return Err(Error::Decompress("back-reference before start of output"));
            }
            let copy_size = (copy_size as usize).min(size - output.len());
            let start = output.len() - distance;
            for i in 0..copy_size {
                output.push(output[start + i]);
            }
        }
    }
    Ok(())
}

/// Inflates a stream without the 8-byte header of archive entries, producing
/// exactly `size` bytes (used inside some formats, e.g. map shadow tiles).
pub fn inflate_raw(input: &[u8], size: usize) -> Result<Vec<u8>> {
    let mut bits = BitReader::new(input, BLOCK_WORDS);
    let mut output = Vec::with_capacity(size);
    inflate_into(&mut bits, size, &mut output)?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dictionary_builds() {
        let _ = dictionary();
    }

    #[test]
    fn truncated_input_is_reported() {
        assert!(matches!(inflate(&[0, 0, 0, 0, 16, 0], None), Err(Error::Truncated)));
        assert_eq!(inflated_size(&[0, 0, 0, 0, 0x34, 0x12, 0, 0]).unwrap(), 0x1234);
    }
}
