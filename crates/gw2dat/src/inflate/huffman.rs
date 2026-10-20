//! Canonical Huffman tree as built by ArenaNet's decoder: an 8-bit lookup table
//! for short codes plus a comparison table for codes of 9..31 bits.

use super::bits::BitSource;
use crate::error::{Error, Result};

pub(crate) const MAX_CODE_BITS: usize = 32;
pub(crate) const MAX_SYMBOLS: usize = 285;
const HASH_BITS: u32 = 8;
const NONE: u16 = u16::MAX;

/// Collects (symbol, code length) pairs. Insertion order matters: within one
/// code length, the symbol added last receives the numerically largest code.
pub(crate) struct HuffmanBuilder {
    head: [u16; MAX_CODE_BITS],
    next: [u16; MAX_SYMBOLS],
}

impl HuffmanBuilder {
    pub fn new() -> Self {
        HuffmanBuilder { head: [NONE; MAX_CODE_BITS], next: [NONE; MAX_SYMBOLS] }
    }

    pub fn clear(&mut self) {
        self.head = [NONE; MAX_CODE_BITS];
        self.next = [NONE; MAX_SYMBOLS];
    }

    pub fn add(&mut self, symbol: u16, bits: u8) -> Result<()> {
        let (symbol_index, bits) = (symbol as usize, bits as usize);
        if symbol_index >= MAX_SYMBOLS || bits >= MAX_CODE_BITS {
            return Err(Error::Decompress("huffman symbol out of range"));
        }
        self.next[symbol_index] = self.head[bits];
        self.head[bits] = symbol;
        Ok(())
    }

    fn is_empty(&self) -> bool {
        self.head.iter().all(|&h| h == NONE)
    }

    /// Returns `None` if no symbol was added.
    pub fn build(&self) -> Result<Option<HuffmanTree>> {
        if self.is_empty() {
            return Ok(None);
        }
        let mut tree = HuffmanTree::empty();
        let mut code: u32 = 0;
        let mut bits: u32 = 0;

        while bits <= HASH_BITS {
            let mut symbol = self.head[bits as usize];
            while symbol != NONE {
                let first = (code << (HASH_BITS - bits)) as u16 as usize;
                let last = (code.wrapping_add(1) << (HASH_BITS - bits)) as u16 as usize;
                if last > tree.hash_symbol.len() {
                    return Err(Error::Decompress("invalid huffman code lengths"));
                }
                for hash in first..last {
                    tree.hash_symbol[hash] = symbol;
                    tree.hash_bits[hash] = bits as u8;
                }
                symbol = self.next[symbol as usize];
                code = code.wrapping_sub(1);
            }
            code = (code << 1).wrapping_add(1);
            bits += 1;
        }

        let mut symbol_count = 0usize;
        while (bits as usize) < MAX_CODE_BITS {
            let mut symbol = self.head[bits as usize];
            if symbol != NONE {
                while symbol != NONE {
                    if symbol_count >= MAX_SYMBOLS {
                        return Err(Error::Decompress("too many huffman symbols"));
                    }
                    tree.symbols[symbol_count] = symbol;
                    symbol_count += 1;
                    symbol = self.next[symbol as usize];
                    code = code.wrapping_sub(1);
                }
                let level = tree.levels;
                tree.code_min[level] = code.wrapping_add(1) << (32 - bits);
                tree.code_bits[level] = bits as u8;
                tree.symbol_offset[level] = (symbol_count - 1) as u16;
                tree.levels += 1;
            }
            code = (code << 1).wrapping_add(1);
            bits += 1;
        }
        Ok(Some(tree))
    }
}

pub(crate) struct HuffmanTree {
    hash_symbol: [u16; 1 << HASH_BITS],
    hash_bits: [u8; 1 << HASH_BITS],
    code_min: [u32; MAX_CODE_BITS],
    code_bits: [u8; MAX_CODE_BITS],
    symbol_offset: [u16; MAX_CODE_BITS],
    symbols: [u16; MAX_SYMBOLS],
    levels: usize,
}

impl HuffmanTree {
    fn empty() -> Self {
        HuffmanTree {
            hash_symbol: [NONE; 1 << HASH_BITS],
            hash_bits: [0; 1 << HASH_BITS],
            code_min: [0; MAX_CODE_BITS],
            code_bits: [0; MAX_CODE_BITS],
            symbol_offset: [0; MAX_CODE_BITS],
            symbols: [0; MAX_SYMBOLS],
            levels: 0,
        }
    }

    pub fn read<B: BitSource>(&self, bits: &mut B) -> Result<u16> {
        let hash = bits.peek(HASH_BITS) as usize;
        let symbol = self.hash_symbol[hash];
        if symbol != NONE {
            bits.consume(self.hash_bits[hash] as u32)?;
            return Ok(symbol);
        }

        let value = bits.peek(32);
        let level = (0..self.levels)
            .find(|&level| value >= self.code_min[level])
            .ok_or(Error::Decompress("invalid huffman code"))?;
        let code_bits = self.code_bits[level] as u32;
        let distance = (value - self.code_min[level]) >> (32 - code_bits);
        let index = (self.symbol_offset[level] as u32)
            .checked_sub(distance)
            .ok_or(Error::Decompress("invalid huffman code"))?;
        let symbol = self.symbols[index as usize];
        bits.consume(code_bits)?;
        Ok(symbol)
    }
}

/// A tree that may degenerate into a single symbol which costs no bits.
pub(crate) enum Code {
    Tree(Box<HuffmanTree>),
    Single(u16),
}

impl Code {
    pub fn read<B: BitSource>(&self, bits: &mut B) -> Result<u16> {
        match self {
            Code::Tree(tree) => tree.read(bits),
            Code::Single(symbol) => Ok(*symbol),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::bits::BitReader;
    use super::*;

    #[test]
    fn canonical_codes_follow_insertion_order() {
        // Two 1-bit symbols: the one added last gets the larger code (1).
        let mut builder = HuffmanBuilder::new();
        builder.add(7, 1).unwrap();
        builder.add(9, 1).unwrap();
        let tree = builder.build().unwrap().unwrap();
        let data = 0x4000_0000u32.to_le_bytes(); // bits: 0, 1, 0, ...
        let mut bits = BitReader::new(&data, 0);
        assert_eq!(tree.read(&mut bits).unwrap(), 7);
        assert_eq!(tree.read(&mut bits).unwrap(), 9);
    }

    #[test]
    fn empty_builder_has_no_tree() {
        assert!(HuffmanBuilder::new().build().unwrap().is_none());
    }

    #[test]
    fn rejects_out_of_range_symbols() {
        assert!(HuffmanBuilder::new().add(MAX_SYMBOLS as u16, 3).is_err());
        assert!(HuffmanBuilder::new().add(1, MAX_CODE_BITS as u8).is_err());
    }
}
