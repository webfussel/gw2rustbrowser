//! Decompression of ATEX-family texture payloads into raw DXT/BC blocks.
//! Port of gw2DatTools' `inflateTextureFileBuffer`.
//!
//! The encoder first marks runs of blocks that share a trivial value (all white,
//! constant alpha, one plain colour); every remaining block is then stored as
//! raw 32-bit words.

use std::sync::OnceLock;

use super::bits::BitSource;
use super::huffman::{HuffmanBuilder, HuffmanTree};
use crate::error::{Error, Result};
use crate::fourcc;

const FF_COLOR: u16 = 0x10;
const FF_ALPHA: u16 = 0x20;
const FF_DEDUCED_ALPHA_COMP: u16 = 0x40;
const FF_PLAIN_COMP: u16 = 0x80;
const FF_BICOLOR_COMP: u16 = 0x200;

const CF_DECODE_WHITE_COLOR: u32 = 0x01;
const CF_DECODE_CONSTANT_ALPHA_FROM_4BITS: u32 = 0x02;
const CF_DECODE_CONSTANT_ALPHA_FROM_8BITS: u32 = 0x04;
const CF_DECODE_PLAIN_COLOR: u32 = 0x08;

#[derive(Clone, Copy)]
struct Layout {
    flags: u16,
    blocks: usize,
    bytes_per_block: usize,
    bytes_per_component: usize,
    two_components: bool,
}

impl Layout {
    fn new(format: u32, width: u16, height: u16) -> Result<Self> {
        let (flags, pixel_bits) = match format {
            fourcc::DXT1 => (FF_COLOR | FF_ALPHA | FF_DEDUCED_ALPHA_COMP, 4),
            // gw2DatTools treats DXTL as a single 16-byte component and so only
            // reads half of each block; the data is laid out exactly like DXT5.
            fourcc::DXT2 | fourcc::DXT3 | fourcc::DXT4 | fourcc::DXT5 | fourcc::DXTL | fourcc::BC7X => {
                (FF_COLOR | FF_ALPHA | FF_PLAIN_COMP, 8)
            }
            fourcc::DXTA => (FF_ALPHA | FF_PLAIN_COMP, 4),
            fourcc::DXTN | fourcc::_3DCX => (FF_BICOLOR_COMP, 8),
            other => return Err(Error::Unsupported(format!("texture format {}", fourcc::display(other)))),
        };
        let blocks = width.div_ceil(4) as usize * height.div_ceil(4) as usize;
        let bytes_per_block = pixel_bits * 4 * 4 / 8;
        let all = FF_PLAIN_COMP | FF_COLOR | FF_ALPHA;
        let two_components = (flags & all) == all || flags & FF_BICOLOR_COMP != 0;
        let bytes_per_component = bytes_per_block / if two_components { 2 } else { 1 };
        Ok(Layout { flags, blocks, bytes_per_block, bytes_per_component, two_components })
    }

    fn output_size(&self) -> usize {
        self.blocks * self.bytes_per_block
    }

    fn color_offset(&self, block: usize) -> usize {
        self.bytes_per_block * block + if self.two_components { self.bytes_per_component } else { 0 }
    }
}

/// Size of the decompressed block data for a texture.
pub fn inflated_texture_size(format: u32, width: u16, height: u16) -> Result<usize> {
    Ok(Layout::new(format, width, height)?.output_size())
}

fn dictionary() -> &'static HuffmanTree {
    static TREE: OnceLock<HuffmanTree> = OnceLock::new();
    TREE.get_or_init(|| {
        let mut builder = HuffmanBuilder::new();
        let mut add = |bits, symbol| builder.add(symbol, bits).expect("static dictionary is valid");
        add(1, 0x01);
        add(2, 0x12);
        for symbol in (0x02..=0x11).rev() {
            add(6, symbol);
        }
        builder.build().expect("static dictionary is valid").expect("static dictionary is not empty")
    })
}

/// Word-based reader that pulls lazily, mirroring gw2DatTools' `State`. The
/// lazy pulling matters: after the bitmap sections the raw block data starts at
/// the first word that was not (fully) consumed.
struct State<'a> {
    data: &'a [u8],
    pos: usize,
    words: usize,
    head: u32,
    buffer: u32,
    bits: u32,
    exhausted: bool,
}

impl<'a> State<'a> {
    fn new(data: &'a [u8]) -> Self {
        State { data, pos: 0, words: data.len() / 4, head: 0, buffer: 0, bits: 0, exhausted: false }
    }

    fn word(&self, index: usize) -> u32 {
        u32::from_le_bytes(self.data[index * 4..index * 4 + 4].try_into().unwrap())
    }

    fn pull(&mut self) -> Result<()> {
        if (self.pos + 1).is_multiple_of(0x4000) {
            self.pos += 1;
        }
        let value = if self.pos >= self.words {
            if self.exhausted {
                return Err(Error::Truncated);
            }
            self.exhausted = true;
            0
        } else {
            self.word(self.pos)
        };
        if self.bits == 0 {
            self.head = value;
            self.buffer = 0;
        } else {
            self.head |= value >> self.bits;
            self.buffer = value << (32 - self.bits);
        }
        self.bits += 32;
        self.pos += 1;
        Ok(())
    }

    fn need(&mut self, n: u32) -> Result<()> {
        if self.bits < n {
            self.pull()?;
        }
        Ok(())
    }

    fn read(&mut self, n: u32) -> Result<u32> {
        self.need(n)?;
        let value = self.peek(n);
        self.consume(n)?;
        Ok(value)
    }

    fn code(&mut self) -> Result<u16> {
        self.need(32)?;
        dictionary().read(self)
    }

    fn next_raw_word(&mut self) -> Option<u32> {
        if self.pos >= self.words {
            return None;
        }
        let value = self.word(self.pos);
        self.pos += 1;
        Some(value)
    }
}

impl BitSource for State<'_> {
    fn peek(&self, n: u32) -> u32 {
        self.head >> (32 - n)
    }

    fn consume(&mut self, n: u32) -> Result<()> {
        if n > self.bits {
            return Err(Error::Truncated);
        }
        if n == 32 {
            self.head = self.buffer;
            self.buffer = 0;
        } else if n > 0 {
            self.head = (self.head << n) | (self.buffer >> (32 - n));
            self.buffer <<= n;
        }
        self.bits -= n;
        Ok(())
    }
}

/// Decompresses a complete ATEX/ATTX/... file into DXT/BC block data.
pub fn inflate_texture(input: &[u8]) -> Result<(u32, u16, u16, Vec<u8>)> {
    let mut state = State::new(input);
    state.read(32)?; // file FourCC
    let format = state.read(32)?;
    state.need(32)?;
    let width = state.read(16)? as u16;
    let height = state.read(16)? as u16;
    let layout = Layout::new(format, width, height)?;
    let mut output = vec![0u8; layout.output_size()];
    inflate_data(&mut state, &layout, &mut output)?;
    Ok((format, width, height, output))
}

/// Decompresses texture block data that has no ATEX header (e.g. in PF files).
pub fn inflate_texture_blocks(input: &[u8], format: u32, width: u16, height: u16) -> Result<Vec<u8>> {
    let layout = Layout::new(format, width, height)?;
    let mut output = vec![0u8; layout.output_size()];
    inflate_data(&mut State::new(input), &layout, &mut output)?;
    Ok(output)
}

fn write(output: &mut [u8], offset: usize, bytes: &[u8]) -> Result<()> {
    output
        .get_mut(offset..offset + bytes.len())
        .ok_or(Error::Decompress("texture block out of range"))?
        .copy_from_slice(bytes);
    Ok(())
}

/// Shared loop of all bitmap sections: alternating run lengths of blocks that
/// do (bit = 1) or do not take the section's value. Blocks already claimed by an
/// earlier section are skipped and do not count.
fn for_each_run(
    state: &mut State,
    layout: &Layout,
    claimed: &mut [bool],
    extra_bit: bool,
    mut apply: impl FnMut(usize, bool) -> Result<bool>,
) -> Result<()> {
    let mut block = 0;
    while block < layout.blocks {
        let mut count = state.code()?;
        let (value, not_null) = if extra_bit {
            state.need(2)?;
            let value = state.peek(1) != 0;
            state.consume(1)?;
            let not_null = state.peek(1) != 0;
            if value {
                state.consume(1)?;
            }
            (value, not_null)
        } else {
            (state.read(1)? != 0, true)
        };
        while count > 0 {
            if block >= layout.blocks {
                // The original reads past its bitmap here; ending the section
                // keeps every block decoded so far.
                return Ok(());
            }
            if !claimed[block] {
                if value && apply(block, not_null)? {
                    claimed[block] = true;
                }
                count -= 1;
            }
            block += 1;
        }
        while block < layout.blocks && claimed[block] {
            block += 1;
        }
    }
    Ok(())
}

fn decode_white(
    state: &mut State,
    layout: &Layout,
    alpha: &mut [bool],
    color: &mut [bool],
    output: &mut [u8],
) -> Result<()> {
    let white = 0xFFFF_FFFF_FFFF_FFFEu64.to_le_bytes();
    let mut newly_claimed = Vec::new();
    for_each_run(state, layout, color, false, |block, _| {
        write(output, layout.bytes_per_block * block, &white)?;
        newly_claimed.push(block);
        Ok(true)
    })?;
    for block in newly_claimed {
        alpha[block] = true;
    }
    Ok(())
}

fn decode_constant_alpha(
    state: &mut State,
    layout: &Layout,
    alpha: &mut [bool],
    output: &mut [u8],
    value: u64,
) -> Result<()> {
    let component = layout.bytes_per_component;
    for_each_run(state, layout, alpha, true, |block, not_null| {
        let bytes = if not_null { value } else { 0 }.to_le_bytes();
        write(output, layout.bytes_per_block * block, &bytes[..component.min(8)])?;
        Ok(true)
    })
}

/// Picks two RGB565 endpoints and an index pattern that best reproduce a single
/// 24-bit colour, exactly as the original encoder expects.
fn plain_color_block(layout: &Layout, red: u32, green: u32, blue: u32) -> u64 {
    let red_1 = ((red - (red >> 5)) >> 3) as u8 as u32;
    let blue_1 = ((blue - (blue >> 5)) >> 3) as u8 as u32;
    let green_1 = (green - (green >> 6)) >> 2;
    let red_2 = (((red_1 << 3) + (red_1 >> 2)) as u8) as i32;
    let blue_2 = (((blue_1 << 3) + (blue_1 >> 2)) as u8) as i32;
    let green_2 = ((green_1 << 2) + (green_1 >> 4)) as i32;

    let comp = |value: u32, rounded: i32, base: u32, mask: u32| -> u32 {
        let divisor = 8 - if base & mask == mask { 1 } else { 0 };
        (12 * (value as i32 - rounded) / divisor) as u32
    };
    let comp_red = comp(red, red_2, red_1, 0x11);
    let comp_blue = comp(blue, blue_2, blue_1, 0x11);
    let comp_green = comp(green, green_2, green_1, 0x1111);

    let split = |comp: u32, base: u32| match comp {
        0..=1 => (base, base),
        2..=5 => (base, base + 1),
        6..=9 => (base + 1, base),
        _ => (base + 1, base + 1),
    };
    let (red_a, red_b) = split(comp_red, red_1);
    let (blue_a, blue_b) = split(comp_blue, blue_1);
    let (green_a, green_b) = split(comp_green, green_1);

    let mut color_a = red_a | ((green_a | (blue_a << 6)) << 5);
    let mut color_b = red_b | ((green_b | (blue_b << 6)) << 5);

    let mut weight = 0u32;
    let mut differing = 0u32;
    for (a, b, base, comp) in
        [(red_a, red_b, red_1, comp_red), (blue_a, blue_b, blue_1, comp_blue), (green_a, green_b, green_1, comp_green)]
    {
        if a != b {
            weight = weight.wrapping_add(if a == base { comp } else { 12u32.wrapping_sub(comp) });
            differing += 1;
        }
    }
    if let Some(average) = weight.wrapping_add(differing / 2).checked_div(differing) {
        weight = average;
    }

    let dxt1_special = layout.flags & FF_DEDUCED_ALPHA_COMP != 0 && (weight == 5 || weight == 6 || differing != 0);
    if differing > 0 && !dxt1_special {
        if color_b == 0xFFFF {
            weight = 12;
            color_a = color_a.wrapping_sub(1);
        } else {
            weight = 0;
            color_b += 1;
        }
    }
    if color_b >= color_a {
        std::mem::swap(&mut color_a, &mut color_b);
        weight = 12u32.wrapping_sub(weight);
    }

    let chosen: u64 = if dxt1_special {
        2
    } else {
        match weight {
            0..=1 => 0,
            2..=5 => 2,
            6..=9 => 3,
            _ => 1,
        }
    };
    let mut indices = chosen | (chosen << 2) | ((chosen | (chosen << 2)) << 4);
    indices |= indices << 8;
    indices |= indices << 16;
    (color_a | color_b.wrapping_shl(16)) as u64 | (indices << 32)
}

fn decode_plain_color(state: &mut State, layout: &Layout, color: &mut [bool], output: &mut [u8]) -> Result<()> {
    state.need(24)?;
    let blue = state.read(8)?;
    let green = state.read(8)?;
    let red = state.read(8)?;
    let value = plain_color_block(layout, red, green, blue).to_le_bytes();
    let component = layout.bytes_per_component.min(8);
    for_each_run(state, layout, color, false, |block, _| {
        write(output, layout.color_offset(block), &value[..component])?;
        Ok(true)
    })
}

fn inflate_data(state: &mut State, layout: &Layout, output: &mut [u8]) -> Result<()> {
    state.head = 0;
    state.buffer = 0;
    state.bits = 0;

    let _data_size = state.read(32)?;
    let flags = state.read(32)?;

    let mut color = vec![false; layout.blocks];
    let mut alpha = vec![false; layout.blocks];

    if flags & CF_DECODE_WHITE_COLOR != 0 {
        decode_white(state, layout, &mut alpha, &mut color, output)?;
    }
    if flags & CF_DECODE_CONSTANT_ALPHA_FROM_4BITS != 0 {
        state.need(4)?;
        let nibble = state.read(4)? as u64;
        let byte = nibble | (nibble << 4);
        let word = byte | (byte << 8);
        let dword = word | (word << 16);
        decode_constant_alpha(state, layout, &mut alpha, output, dword | (dword << 32))?;
    }
    if flags & CF_DECODE_CONSTANT_ALPHA_FROM_8BITS != 0 {
        state.need(8)?;
        let byte = state.read(8)? as u64;
        decode_constant_alpha(state, layout, &mut alpha, output, byte | (byte << 8))?;
    }
    if flags & CF_DECODE_PLAIN_COLOR != 0 {
        decode_plain_color(state, layout, &mut color, output)?;
    }

    // Un-read the word that is still fully buffered.
    if state.bits >= 32 {
        state.pos -= 1;
    }

    let has_alpha = (layout.flags & FF_ALPHA != 0 && layout.flags & FF_DEDUCED_ALPHA_COMP == 0)
        || layout.flags & FF_BICOLOR_COMP != 0;
    if has_alpha {
        for (block, _) in alpha.iter().enumerate().filter(|(_, claimed)| !**claimed) {
            let Some(word) = state.next_raw_word() else { break };
            let offset = layout.bytes_per_block * block;
            write(output, offset, &word.to_le_bytes())?;
            if layout.bytes_per_component > 4 {
                let Some(word) = state.next_raw_word() else { break };
                write(output, offset + 4, &word.to_le_bytes())?;
            }
        }
    }

    if layout.flags & (FF_COLOR | FF_BICOLOR_COMP) != 0 {
        for (block, _) in color.iter().enumerate().filter(|(_, claimed)| !**claimed) {
            let Some(word) = state.next_raw_word() else { break };
            write(output, layout.color_offset(block), &word.to_le_bytes())?;
        }
        if layout.bytes_per_component > 4 {
            for (block, _) in color.iter().enumerate().filter(|(_, claimed)| !**claimed) {
                let Some(word) = state.next_raw_word() else { break };
                write(output, layout.color_offset(block) + 4, &word.to_le_bytes())?;
            }
        }
    }
    Ok(())
}
