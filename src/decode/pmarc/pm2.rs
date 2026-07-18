//! PMarc v2 decoder
//!
//! Original C version: (c) 2011, 2012, Simon Howard lhasa/lib/pm2_decoder.c
//!
//! Rust version: (c) 2026, Rafał Michalski
#![allow(dead_code)]
#![allow(unused_imports)]
#[cfg(not(feature = "std"))]
use alloc::boxed::Box;
use crate::statictree::HuffTree;
use core::num::NonZeroU16;
use crate::{
    bitstream::*,
    decode::Decoder,
    error::{LhaResult, DecompressionError},
    ringbuf::*,
};
use bytemuck::allocation::zeroed_box;
use super::*;

const RING_BUFFER_SIZE: usize = 8192;

/// Maximum number of leaf nodes in the code tree
const NUM_COMMANDS: usize = 29;

/// Maximum number of leaf nodes in the offset tree
const NUM_OFFSET_LEMENTS: usize =  8;

/// State of the decode trees
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
enum RebuildState {
    /// Start of stream, no data read yet.
    #[default]
    Unbuilt,
    /// Until 1KiB is output
    Build1k,
    /// Until 2KiB is output
    Build2k,
    /// Until 4KiB is output
    Build4k,
    /// 8KiB was output
    Continuing,
}

/// A decoder for `-pm2-` compression method.
#[derive(Debug)]
pub struct Pm2Decoder<R> {
    bit_reader: BitStream<R>,
    copy_progress: Option<(u16, NonZeroU16)>,
    tree_state: RebuildState,
    /// Number of bytes until we initiate a tree rebuild
    tree_rebuild_remaining: u16,
    ringbuf: Box<RingArrayBuf<RING_BUFFER_SIZE>>,
    history_list: Box<HistoryLinkedList>,
    command_tree: HuffTree,
    offset_tree: HuffTree,
    need_offset_tree: bool
}


impl<R: Read> Pm2Decoder<R> {
    /// Create a new decoder instance from the given data read stream
    pub fn new(rd: R) -> Pm2Decoder<R> {
        let bit_reader = BitStream::new(rd);
        let mut ringbuf = zeroed_box::<RingArrayBuf<RING_BUFFER_SIZE>>();
        ringbuf.initialize(b' ');
        let history_list = HistoryLinkedList::new_boxed();
        let command_tree = HuffTree::with_leaf_capacity(NUM_COMMANDS);
        let offset_tree = HuffTree::with_leaf_capacity(NUM_OFFSET_LEMENTS);
        Pm2Decoder {
            bit_reader,
            copy_progress: None,
            tree_state: Default::default(),
            tree_rebuild_remaining: 0,
            ringbuf,
            history_list,
            command_tree,
            offset_tree,
            need_offset_tree: false
        }
    }

    fn copy_from_history<'a, I: ExactSizeIterator<Item=&'a mut u8>>(
            &mut self,
            mut target: I,
            offset: u16,
            count: u16
        ) -> LhaResult<(), R>
    {
        let mut pending_count = target.len().min(count.into());
        // pending_count <= count
        let count_after = count - pending_count as u16;
        while pending_count != 0 {
            let history_iter = self.ringbuf.iter_from_offset(offset.into());
            let pass_count = pending_count.min(self.tree_rebuild_remaining.into());
            for (t, s) in target.by_ref().zip(history_iter).take(pass_count) {
                *t = s;
                // update history linked list
                self.history_list.update_history_list(s);
            }
            self.tree_rebuild_remaining -= pass_count as u16;
            if self.tree_rebuild_remaining == 0 {
                self.rebuild_tree()?;
            }
            pending_count -= pass_count;
        }
        self.copy_progress = NonZeroU16::new(count_after)
                            .map(|count| (offset, count));
        Ok(())
    }

    /// Read the list of code lengths to use for the code tree and construct
    /// the command_tree structure
    fn read_command_tree(&mut self) -> LhaResult<(), R> {
        let mut code_lengths = [0u8; NUM_COMMANDS];

        // number of codes to read
        let num_codes: usize = self.bit_reader.read_bits(5)?;

        // Simon Howard:
        // Code values > 28 can potentially lead to an overflow of the
        // copy_decode table (GHSA-j2m3-h278-rrg9). We prevent this by
        // checking there are no more than 29 codes (the 28/29
        // difference here is not an error; 0-28 is 29 codes).
        if num_codes > NUM_COMMANDS {
            return Err(LhaError::Decompress(DecompressionError::CommandCodeTableOverflow))
        }

        // read min_code_length, which is used as a length base.
        let min_code_length: u8 = self.bit_reader.read_bits(3)?;

        // do we need the offset tree?
        self.need_offset_tree = num_codes >= 10 &&
                              !(num_codes == NUM_COMMANDS && min_code_length == 0);

        // minimum length of zero means a tree contains a single code
        if min_code_length == 0 {
            let code = num_codes.checked_sub(1)
                      .ok_or(LhaError::Decompress(DecompressionError::CommandOverflow))?;
            self.command_tree.set_single(code as u16);
            return Ok(());
        }

        // How many bits are used to represent each table entry?
        let length_bits: u32 = self.bit_reader.read_bits(3)?; // 0..=7

        // Read table of code lengths
        for p in code_lengths[0..num_codes].iter_mut() {
            // Read a table entry.  A value of zero represents an
            // unused code.  Otherwise the value represents
            // an offset from the minimum length (previously read).
            let val: u8 = self.bit_reader.read_bits(length_bits)?; // 0..=127
            if val != 0 {
                *p = min_code_length + val - 1;
            }
        }

        // Build the tree.
        self.command_tree.build_tree(&code_lengths[0..num_codes])?;
        Ok(())
    }

    /// Read the code lengths for the offset tree and construct the offset
    /// tree lookup table
    fn read_offset_tree(&mut self, num_offsets: usize) -> LhaResult<(), R> {
        let mut offset_lengths = [0u8; NUM_OFFSET_LEMENTS];

        // check sanity of the caller
        assert!(num_offsets <= NUM_OFFSET_LEMENTS);

        if !self.need_offset_tree {
            return Ok(());
        }

        // Read 'num_offsets' 3-bit length values.  For each offset
        // value 'off', offset_lengths[off] is the length of the
        // code that will represent 'off', or 0 if it will not
        // appear within the tree.
        let mut num_codes = 0usize;
        let mut single_offset = 0;

        for (p, off) in offset_lengths[0..num_offsets].iter_mut().zip(0u16..) {
            let len: u8 = self.bit_reader.read_bits(3)?;
            *p = len;

            // Track how many actual codes were in the tree.
            if len != 0 {
                single_offset = off;
                num_codes += 1;
            }
        }

        // If there was a single code, this is a single node tree.
        if num_codes == 1 {
            self.offset_tree.set_single(single_offset);
            return Ok(());
        }

        // Build the tree.
        self.offset_tree.build_tree(&offset_lengths[0..num_offsets])?;
        return Ok(())
    }

    // Rebuild the decode trees used to compress data.  This is called when
    // decoder->tree_rebuild_remaining reaches zero.
    fn rebuild_tree(&mut self) -> LhaResult<(), R> {
        match self.tree_state {
            // initial tree build, from the start of stream
            RebuildState::Unbuilt => {
                self.read_command_tree()?;
                self.read_offset_tree(5)?;
                self.tree_state = RebuildState::Build1k;
                self.tree_rebuild_remaining = 1024;
            }
            // build after 1KiB of data has been read
            RebuildState::Build1k => {
                self.read_offset_tree(6)?;
                self.tree_state = RebuildState::Build2k;
                self.tree_rebuild_remaining = 1024;
            }
            // build after 2KiB of data has been read
            RebuildState::Build2k => {
                self.read_offset_tree(7)?;
                self.tree_state = RebuildState::Build4k;
                self.tree_rebuild_remaining = 2048;
            }
            // build after 4KiB of data has been read
            RebuildState::Build4k => {
                if self.bit_reader.read_bit()? {
                    self.read_command_tree()?;
                }
                self.read_offset_tree(8)?;
                self.tree_state = RebuildState::Continuing;
                self.tree_rebuild_remaining = 4096;
            }
            // build after 8KiB of data has been read,
            // and every 4KiB after that
            RebuildState::Continuing => {
                if self.bit_reader.read_bit()? {
                    self.read_command_tree()?;
                    self.read_offset_tree(8)?;
                }
                self.tree_rebuild_remaining = 4096;
            }
        }
        Ok(())
    }

    // Read a single byte from the input stream
    fn read_single_byte(&mut self, code: u16) -> LhaResult<u8, R> {
        // Simon Howard:
        // Decode table for history value. Characters that appeared recently in
        // the history are more likely than ones that appeared a long time ago,
        // so the history value is huffman coded so that small values require
        // fewer bits. The history value is then used to search within the
        // history linked list to get the actual character.
        type E = VarLenEntry;
        const HISTORY_DECODE: [VarLenEntry;8] = [
            E::new(   0, 3 ),   //   0 + (1 << 3) =   8
            E::new(   8, 3 ),   //   8 + (1 << 3) =  16
            E::new(  16, 4 ),   //  16 + (1 << 4) =  32
            E::new(  32, 5 ),   //  32 + (1 << 5) =  64
            E::new(  64, 5 ),   //  64 + (1 << 5) =  96
            E::new(  96, 5 ),   //  96 + (1 << 5) = 128
            E::new( 128, 6 ),   // 128 + (1 << 6) = 192
            E::new( 192, 6 ),   // 192 + (1 << 6) = 256
        ];

        let offset = HISTORY_DECODE[usize::from(code)]
                    .decode_variable_length(&mut self.bit_reader)?;
        debug_assert!(offset <= u16::from(u8::MAX));
        let byte = self.history_list.find_in_history_list(offset as u8);
        Ok(byte)
    }


    // Calculate how many bytes from history to copy:
    fn history_get_count(&mut self, code: u16) -> LhaResult<u16, R> {
        // Decode table for copies. As with history_decode[], small copies
        // are more common, and require fewer bits.
        type E = VarLenEntry;
        const COPY_DECODE: [VarLenEntry;6] = [
            E::new(  17, 3 ),   //  17 + (1 << 3) =  25
            E::new(  25, 3 ),   //  25 + (1 << 3) =  33
            E::new(  33, 5 ),   //  33 + (1 << 5) =  65
            E::new(  65, 6 ),   //  65 + (1 << 6) = 129
            E::new( 129, 7 ),   // 129 + (1 << 7) = 256
            E::new( 256, 0 ),   // 256 (unique value)
        ];
        // How many bytes to copy?  A small value represents the
        // literal number of bytes to copy; larger values are a header
        // for a variable length value to be decoded.
        if code < 15 {
            Ok(code + 2)
        }
        else {
            COPY_DECODE[usize::from(code - 15)].decode_variable_length(&mut self.bit_reader)
        }
    }


    // Calculate the offset within history at which to start copying
    fn history_get_offset(&mut self, code: u16) -> LhaResult<u16, R> {
        let mut result = 0u16;

        // calculate number of bits to read
        let bits = match code {
            // Zero indicates a simple 6-bit value giving the offset.
            // xxxxxx
            0 => 6,
            // Mid-range encoded offset value from the offset tree.
            // The value indicates the number of bits:
            // Values 0-7 = 6-13 bits with a highest bit always set.
            // 1xxxxxx
            // 1xxxxxxx
            // ...
            // 1xxxxxxxxxxxx
            1..=19 => {
                let val = self.offset_tree.read_entry(&mut self.bit_reader)?;
                if val == 0 {
                    6
                }
                else {
                    let bits = u32::from(val) + 5;
                    result = 1u16 << bits;
                    bits
                }
            }
            // Large copy values start from offset zero.
            _ => return Ok(0)
        };

        // Read a number of bits representing the offset value.  The
        // number of length of this value is variable, and is calculated
        // above.
        let val: u16 = self.bit_reader.read_bits(bits)?;
        Ok(result | val)
    }

    #[inline]
    fn read_command(&mut self) -> LhaResult<u16, R> {
        self.command_tree.read_entry(&mut self.bit_reader)
    }
}

impl<R: Read> Decoder<R> for Pm2Decoder<R> where R::Error: core::error::Error {
    type Error = R::Error;

    fn into_inner(self) -> R {
        self.bit_reader.into_inner()
    }

    fn fill_buffer(&mut self, buf: &mut[u8]) -> LhaResult<(), R> {
        let buflen = buf.len();
        let mut target = buf.iter_mut();
        if let Some((offset, count)) = self.copy_progress {
            self.copy_from_history(&mut target, offset, count.get())?;
        }
        // build initial lookup trees
        else if self.tree_state == RebuildState::Unbuilt {
            // first bit in stream is discarded?
            self.bit_reader.read_bit()?;
            self.rebuild_tree()?;
        }

        while let Some(dst) = target.next() {
            match self.read_command()? {
                code @ 0..=7 => {
                    let byte = self.read_single_byte(code)?;
                    *dst = byte;
                    // add to history ring buffer
                    self.ringbuf.push(byte);
                    // update history linked list
                    self.history_list.update_history_list(byte);
                    // count down until it is time to rebuild a tree
                    self.tree_rebuild_remaining -= 1;
                    if self.tree_rebuild_remaining == 0 {
                        self.rebuild_tree()?;
                    }
                }
                code => {
                    let code = code - 8; // 8..=20
                    assert!(code < 21);
                    // read the number of bytes to copy and history offset
                    let count = self.history_get_count(code)?;
                    let offset = self.history_get_offset(code)?;
                    let index = buflen - target.len() - 1;
                    target = buf[index..].iter_mut();
                    self.copy_from_history(&mut target, offset, count)?
                }
            }
        }
        Ok(())
    }
}

#[cfg(feature = "std")]
#[cfg(test)]
mod tests {
    use std::{io, fs, time::{Instant, Duration}};
    use super::*;

    #[test]
    fn pmarc2_works() {
        println!("Pm2Decoder<Empty> {}", size_of::<Pm2Decoder<io::Empty>>());
        println!("Pm2Decoder<File> {}", size_of::<Pm2Decoder<fs::File>>());
        println!("RingArrayBuf<RING_BUFFER_SIZE> {}", size_of::<RingArrayBuf<RING_BUFFER_SIZE>>());
        let _ = Pm2Decoder::new(io::empty());
    }
}
