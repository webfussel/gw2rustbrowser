//! MSB-first bit reader over little-endian 32-bit words, as used by ArenaNet's
//! compression. Optionally skips every `period`-th word, which is how the
//! 4-byte checksum at the end of every 64 KiB block is hidden from the stream.

use crate::error::{Error, Result};

pub(crate) trait BitSource {
    /// Top `n` (1..=32) bits of the stream, without consuming them. Bits past
    /// the end of the input read as zero.
    fn peek(&self, n: u32) -> u32;
    /// Consumes `n` (0..=32) bits.
    fn consume(&mut self, n: u32) -> Result<()>;

    fn take(&mut self, n: u32) -> Result<u32> {
        if n == 0 {
            return Ok(0);
        }
        let value = self.peek(n);
        self.consume(n)?;
        Ok(value)
    }
}

pub(crate) struct BitReader<'a> {
    data: &'a [u8],
    /// Index of the next word to load.
    word: usize,
    words: usize,
    skip_period: usize,
    /// Bit window, MSB-aligned. Bits below `bits` are always zero.
    acc: u64,
    bits: u32,
}

impl<'a> BitReader<'a> {
    pub fn new(data: &'a [u8], skip_period: usize) -> Self {
        let mut reader = BitReader { data, word: 0, words: data.len() / 4, skip_period, acc: 0, bits: 0 };
        reader.refill();
        reader
    }

    fn next_word(&mut self) -> Option<u32> {
        if self.skip_period != 0 && (self.word + 1).is_multiple_of(self.skip_period) {
            self.word += 1;
        }
        if self.word >= self.words {
            return None;
        }
        let offset = self.word * 4;
        self.word += 1;
        Some(u32::from_le_bytes(self.data[offset..offset + 4].try_into().unwrap()))
    }

    fn refill(&mut self) {
        while self.bits <= 32 {
            match self.next_word() {
                Some(word) => {
                    self.acc |= (word as u64) << (32 - self.bits);
                    self.bits += 32;
                }
                None => break,
            }
        }
    }
}

impl BitSource for BitReader<'_> {
    #[inline]
    fn peek(&self, n: u32) -> u32 {
        debug_assert!((1..=32).contains(&n));
        (self.acc >> (64 - n)) as u32
    }

    #[inline]
    fn consume(&mut self, n: u32) -> Result<()> {
        if n > self.bits {
            return Err(Error::Truncated);
        }
        self.acc = if n == 64 { 0 } else { self.acc << n };
        self.bits -= n;
        if self.bits <= 32 {
            self.refill();
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn words(values: &[u32]) -> Vec<u8> {
        values.iter().flat_map(|v| v.to_le_bytes()).collect()
    }

    #[test]
    fn reads_msb_first_across_words() {
        let data = words(&[0xABCD_1234, 0x5678_9ABC]);
        let mut bits = BitReader::new(&data, 0);
        assert_eq!(bits.take(8).unwrap(), 0xAB);
        assert_eq!(bits.take(16).unwrap(), 0xCD12);
        assert_eq!(bits.take(16).unwrap(), 0x3456);
        assert_eq!(bits.take(24).unwrap(), 0x789ABC);
        assert!(matches!(bits.take(1), Err(Error::Truncated)));
    }

    #[test]
    fn skips_checksum_word_of_each_block() {
        // Period 4: word index 3 (the "checksum") is never seen.
        let data = words(&[1, 2, 3, 0xDEAD, 5]);
        let mut bits = BitReader::new(&data, 4);
        let read: Vec<u32> = (0..4).map(|_| bits.take(32).unwrap()).collect();
        assert_eq!(read, [1, 2, 3, 5]);
    }
}
