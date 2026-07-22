//! PMarc -pm1- decoder
//!
//! Original C version: (c) 2011, 2012, Simon Howard lhasa/lib/pm1_decoder.c
//!
//! Rust version: (c) 2026, Rafał Michalski
#[cfg(not(feature = "std"))]
use alloc::boxed::Box;
use core::num::NonZeroU8;
use bytemuck::allocation::zeroed_box;
use crate::{
    bitstream::*,
    decode::Decoder,
    error::{LhaResult, DecompressionError},
    ringbuf::*,
};
use super::*;

const RING_BUFFER_SIZE: usize = 16384;

/// Maximum length of a command representing a block of bytes
const MAX_BYTE_BLOCK_LEN: u8 = 216;

/// Operation in progress
#[derive(Debug, Clone, Copy)]
enum Progress {
    /// Reading data block
    Read { count: NonZeroU8, copy_next: bool },
    /// Copying from history
    Copy { count: NonZeroU8, offset: u16 },
}

/// A decoder for `-pm1-` compression method.
#[derive(Debug)]
pub struct Pm1Decoder<R> {
    bit_reader: BitStream<NoEofReader<R>>,
    progress: Option<Progress>,
    ringbuf: Box<RingArrayBuf<RING_BUFFER_SIZE>>,
    history_list: Box<HistoryLinkedList>,
    /// saturating count of output bytes,
    /// the range of history offset depends on this
    output_stream_pos: u16,
    /// this needs to be read at the start of file
    byte_decode_tree: [u8;5],
    byte_decode_tree_ready: bool,
}

// Simon Howard:
// This table is a list of trees to decode indices into byte_ranges.
// Each line is actually a mini binary tree, starting with the first
// byte as the root node. Each nybble of the byte is one of the two
// branches: either a leaf value (a-f) or an offset to the child node.
// Expanded representation is shown in comments below.
static BYTE_DECODE_TREES: [[u8;5];32] = [
       [ 0x12, 0x2d, 0xef, 0x1c, 0xab ],    // ((((a b) c) d) (e f))
       [ 0x12, 0x23, 0xde, 0xab, 0xcf ],    // (((a b) (c f)) (d e))
       [ 0x12, 0x2c, 0xd2, 0xab, 0xef ],    // (((a b) c) (d (e f)))
       [ 0x12, 0xa2, 0xd2, 0xbc, 0xef ],    // ((a (b c)) (d (e f)))

       [ 0x12, 0xa2, 0xc2, 0xbd, 0xef ],    // ((a (b d)) (c (e f)))
       [ 0x12, 0xa2, 0xcd, 0xb1, 0xef ],    // ((a (b (e f))) (c d))
       [ 0x12, 0xab, 0x12, 0xcd, 0xef ],    // ((a b) ((c d) (e f)))
       [ 0x12, 0xab, 0x1d, 0xc1, 0xef ],    // ((a b) ((c (e f)) d))

       [ 0x12, 0xab, 0xc1, 0xd1, 0xef ],    // ((a b) (c (d (e f))))
       [ 0xa1, 0x12, 0x2c, 0xde, 0xbf ],    // (a (((b f) c) (d e)))
       [ 0xa1, 0x1d, 0x1c, 0xb1, 0xef ],    // (a (((b (e f)) c) d))
       [ 0xa1, 0x12, 0x2d, 0xef, 0xbc ],    // (a (((b c) d) (e f)))

       [ 0xa1, 0x12, 0xb2, 0xde, 0xcf ],    // (a ((b (c f)) (d e)))
       [ 0xa1, 0x12, 0xbc, 0xd1, 0xef ],    // (a ((b c) (d (e f))))
       [ 0xa1, 0x1c, 0xb1, 0xd1, 0xef ],    // (a ((b (d (e f))) c))
       [ 0xa1, 0xb1, 0x12, 0xcd, 0xef ],    // (a (b ((c d) (e f))))

       [ 0xa1, 0xb1, 0xc1, 0xd1, 0xef ],    // (a (b (c (d (e f)))))
       [ 0x12, 0x1c, 0xde, 0xab, 0    ],    // (((d e) c) (d e)) <- BROKEN!
       [ 0x12, 0xa2, 0xcd, 0xbe, 0    ],    // ((a (b e)) (c d))
       [ 0x12, 0xab, 0xc1, 0xde, 0    ],    // ((a b) (c (d e)))

       [ 0xa1, 0x1d, 0x1c, 0xbe, 0    ],    // (a (((b e) c) d))
       [ 0xa1, 0x12, 0xbc, 0xde, 0    ],    // (a ((b c) (d e)))
       [ 0xa1, 0x1c, 0xb1, 0xde, 0    ],    // (a ((b (d e)) c))
       [ 0xa1, 0xb1, 0xc1, 0xde, 0    ],    // (a (b (c (d e))))

       [ 0x1d, 0x1c, 0xab, 0   , 0    ],    // (((a b) c) d)
       [ 0x1c, 0xa1, 0xbd, 0   , 0    ],    // ((a (b d)) c)
       [ 0x12, 0xab, 0xcd, 0   , 0    ],    // ((a b) (c d))
       [ 0xa1, 0x1c, 0xbd, 0   , 0    ],    // (a ((b d) c))

       [ 0xa1, 0xb1, 0xcd, 0   , 0    ],    // (a (b (c d)))
       [ 0xa1, 0xbc, 0   , 0   , 0    ],    // (a (b c))
       [ 0xab, 0   , 0   , 0   , 0    ],    // (a b)
       [ 0x00; 5 ],                         // -- special entry: 0, no tree
];

// FIXME: u16::bit_width() MRV 1.97
#[inline(always)]
const fn bit_width(v: u16) -> u16 {
    (u16::BITS - v.leading_zeros()) as u16
}

impl<R: Read> Pm1Decoder<R> {
    /// Create a new decoder instance from the given data read stream
    pub fn new(rd: R) -> Pm1Decoder<R> {
        let bit_reader = BitStream::new(NoEofReader(rd));
        let ringbuf = zeroed_box::<RingArrayBuf<RING_BUFFER_SIZE>>();
        let history_list = HistoryLinkedList::new_boxed();
        Pm1Decoder {
            bit_reader,
            ringbuf,
            progress: None,
            history_list,
            output_stream_pos: 0,
            byte_decode_tree_ready: false,
            byte_decode_tree: Default::default()
        }
    }

    /// Read the 5-bit header from the start of the input stream. This
    /// specifies the table entry to use for byte decodes.
    fn read_start_header(&mut self) -> LhaResult<(), R> {
        let index: usize = self.bit_reader.read_bits(5)?;
        self.byte_decode_tree = BYTE_DECODE_TREES[index & 0x1F];
        self.byte_decode_tree_ready = true;
        Ok(())
    }

    /// Progressively copy data from history buffer
    fn copy_from_history<'a, I: ExactSizeIterator<Item=&'a mut u8>>(
            &mut self,
            target: I,
            offset: u16,
            count: u8
        )
    {
        let history_iter = self.ringbuf.iter_from_offset(offset.into());
        let actual_count = target.len().min(count.into());
        for (t, s) in target.zip(history_iter).take(actual_count) {
            *t = s;
            // update history linked list, output stream position
            self.history_list.update_history_list(s);
        }
        // actual_count <= count
        self.output_stream_pos = self.output_stream_pos.saturating_add(actual_count as u16);
        let count_after = count - actual_count as u8;
        // offset can be truncated
        self.progress = NonZeroU8::new(count_after)
                        .map(|count| Progress::Copy { count, offset });
    }

    /// Progressively read data block to the end
    fn read_byte_block<'a, I: ExactSizeIterator<Item=&'a mut u8>>(
            &mut self,
            mut target: I,
            count: u8,
            copy_next: bool
        ) -> LhaResult<(), R>
    {
        let actual_count = target.len().min(count.into());
        for t in target.by_ref().take(count.into()) {
            let byteval = self.read_byte()?;
            *t = byteval;
            // add to history ring buffer.
            self.ringbuf.push(byteval);
            // update history linked list, output stream position
            self.history_list.update_history_list(byteval);
        }
        // actual_count <= count
        self.output_stream_pos = self.output_stream_pos.saturating_add(actual_count as u16);
        let count_after = count - actual_count as u8;
        self.progress = NonZeroU8::new(count_after)
                        .map(|count| Progress::Read { count, copy_next });

        if copy_next && count_after == 0 {
            return self.read_copy_command(target)
        }
        Ok(())
    }

    /// Decode the number of bytes to copy in a copy command.
    ///
    /// The returned value is in the range: 3..=244.
    fn read_copy_byte_count(&mut self) -> LhaResult<u8, R> {
        // Simon Howard:
        // This is a form of static huffman encoding that uses less bits
        // to encode short copy amounts (again).

        // .2 <3 (+3)
        //    3:
        //       .3 <5 (+6)
        //          5: .2 (+11)
        //          6: .3 (+15)
        //          7:
        //             .6 <62 (+23)
        //                62: .5 (+85)
        //                63: .7 (+117)

        // value in the range 3..=5?
        // Simon Howard:
        // Length values start at 3: if it was 2, a different copy
        // range would have been used and this function would not
        // have been called.

        let x: u8 = self.bit_reader.read_bits(2)?;

        if x < 3 {
            return Ok(x + 3)
        }

        let next = match self.bit_reader.read_bits(3)? {
            7 => None,
            // value in the range 15..=22?
            6 => Some((3, 15)),
            // value in the range 11..=14?
            5 => Some((2, 11)),
            // value in the range 6..=10?
            x => return Ok(x + 6), // x < 5
        };

        if let Some((bits, offset)) = next {
            return self.bit_reader.read_bits(bits).map(|x: u8| x + offset)
        }

        let (bits, offset) = match self.bit_reader.read_bits(6)? {
            // value in the range 117..=244?
            63 => (7, 117), // x == 63
            // value in the range 85..=116?
            62 => (5, 85),
            // value in the range 23..=84?
            x  => return Ok(x + 23),
        };

        self.bit_reader.read_bits(bits).map(|x: u8| x + offset)
    }

    // /// Read a single bit from the input stream, but only once the specified
    // /// point is reached in the output stream. Before that point is reached,
    // /// return the value of 'def' instead.
    // fn read_bit_after_threshold(&mut self, threshold: u16, def: bool) -> LhaResult<bool, R> {
    //     if self.output_stream_pos >= threshold {
    //         self.bit_reader.read_bit()
    //     }
    //     else {
    //         Ok(def)
    //     }
    // }

    /// Read the range index for the copy type used when performing a copy command.
    ///
    /// The returned value is in the range: 0..=5.
    fn read_copy_type_range(&mut self) -> LhaResult<usize, R> {
        // Simon Howard:
        // This is another static huffman tree, but the path grows as
        // more data is decoded. The progression is as follows:
        //  1. Initially, only '0' and '2' can be returned.
        //  2. After 64 bytes, '1' and '3' can be returned as well.
        //  3. After 576 bytes, '4' can be returned.
        //  4. After 2624 bytes, '5' can be returned.

        // t <   64: 0b0   -> 0,                                              0b1   -> 2
        // t <  576: 0b00  -> 0, 0b01  -> 1,            0b10 -> 3,            0b11  -> 2
        // t < 2264: 0b000 -> 0, 0b001 -> 1, 0b01 -> 4, 0b10 -> 3,            0b11  -> 2
        // t       : 0b000 -> 0, 0b001 -> 1, 0b01 -> 4, 0b10 -> 3, 0b110 ->5, 0b111 -> 2

        // let range_index = if !self.bit_reader.read_bit()? {
        //     if self.read_bit_after_threshold(576, false)? {
        //         4 // 0b01 (>=576)
        //     }
        //     else {
        //         // Return either 0 or 1.
        //         self.read_bit_after_threshold(64, false).map(Into::into)?
        //         // 0 0b000 (>=576) or 0b00 (>=64) or 0b0
        //         // 1 0b001 (>=576) or 0b01 (>=64)
        //     }
        // }
        // else {
        //     if !self.read_bit_after_threshold(64, true)? {
        //         3 // 0b10 (>=64)
        //     }
        //     else if self.read_bit_after_threshold(2624, true)? {
        //         2 // 0b111 (>=2624) or 0b11 (>= 64) or 0b1
        //     }
        //     else {
        //         5 // 0b110 (>=2624)
        //     }
        // };

        let t = self.output_stream_pos;
        let range_index = if t < 64 {
            (self.bit_reader.read_bits::<usize>(1)? & 1) * 2 // 0 or 2
        }
        else {
            let x = self.bit_reader.read_bits::<usize>(2)?;
            if t < 576 {
                ((x >> 1) ^ x) & 3 // 0b00: 0, 0b01: 1, 0b10: 3, 0b11: 2
            }
            else {
                match x {
                    0b00 => self.bit_reader.read_bits::<usize>(1)? & 1, // 0b000: 0, 0b001: 1
                    0b01 => 4,
                    0b10 => 3,
                    // 0b11
                    _ => if t < 2624 || self.bit_reader.read_bit()? {
                        2 // 0b11 or 0b111
                    }
                    else {
                        5 // 0b110
                    }
                }
            }
        };
        Ok(range_index)
    }

    /// Read a copy command from the input stream and copy from history.
    ///
    /// Return the number of bytes copied.
    fn read_copy_command<'a, I: ExactSizeIterator<Item=&'a mut u8>>(
            &mut self,
            target: I
        ) -> LhaResult<(), R>
    {
        let range_index = self.read_copy_type_range()?;

        // Simon Howard:
        // The first two entries in the copy_ranges table are used as
        // a shorthand to copy two bytes. Otherwise, decode the number
        // of bytes to copy.
        let count = if range_index < 2 {
            2
        }
        else {
            self.read_copy_byte_count()?
        };

        type E = VarLenEntry;
        const COPY_RANGES: [VarLenEntry;6] = [
            E::new(    0,  6 ),  //    0 +  (1 << 6) - 1 =    63
            E::new(   64,  8 ),  //   64 +  (1 << 8) - 1 =   319
            E::new(    0,  6 ),  //    0 +  (1 << 6) - 1 =    63
            E::new(   64,  9 ),  //   64 +  (1 << 9) - 1 =   575
            E::new(  576, 11 ),  //  576 + (1 << 11) - 1 =  2623
            E::new( 2624, 13 ),  // 2624 + (1 << 13) - 1 = 10815

            // Simon Howard:
            // The above table entries are used after a certain number of
            // bytes have been decoded.
            // Early in the stream, some of the copy ranges are more limited
            // in their range, so that fewer bits are needed. The above
            // table entries are redirected to these entries instead.
            // Table entry #3 (64):
            /*
            E::new(   64,  8 ),   // < 320 bytes   (320-64)   bits(< 256)  <=8

            // Table entry #4 (576):
            E::new(  576,  8 ),   // < 832  bytes  ( 832-576) bits(< 256)  <=8
            E::new(  576,  9 ),   // < 1088 bytes  (1088-576) bits(< 512)  = 9
            E::new(  576, 10 ),   // < 1600 bytes  (1600-576) bits(< 1024) = 10

            // Table entry #5 (2624):
            E::new( 2624,  8 ),   // < 2880 bytes (2880-2624) bits(< 256)  <=8
            E::new( 2624,  9 ),   // < 3136 bytes (3136-2624) bits(< 512)  = 9
            E::new( 2624, 10 ),   // < 3648 bytes (3648-2624) bits(< 1024) = 10
            E::new( 2624, 11 ),   // < 4672 bytes (4672-2624) bits(< 2048) = 11
            E::new( 2624, 12 ),   // < 6720 bytes (6720-2624) bits(< 4096) = 12
            */
        ];

        let pos = self.output_stream_pos;

        // Simon Howard:
        // The 'range_index' variable is an index into the copy_ranges
        // array. As a special-case hack, early in the output stream
        // some history ranges are inaccessible, so fewer bits can be
        // used. Redirect range_index to special entries to do this.
        /*
        let range_index = match range_index {
            3 if pos < 320 => 6,
            4 => match pos {
                   0..832  => 7,
                 832..1088 => 8,
                1088..1600 => 9,
                _ => range_index
            },
            5 => match pos {
                   0..2880 => 10,
                2880..3136 => 11,
                3136..3648 => 12,
                3648..4672 => 13,
                4672..6720 => 14,
                _ => range_index
            },
            _ => range_index
        };
        */
        let mut range = COPY_RANGES[range_index];

        // RM: Instead of the extra table entries here we estimate range
        // bit size from the output position bit width
        if range.bits > 8 { // 9, 11, 13
            // limits the bits - between 8 and range.bits depending on
            // the bit width of the (stream position - range.offs)
            // this exactly matches the original algorithm
            range.bits = bit_width(pos.saturating_sub(range.offs))
                        .clamp(8, range.bits);
        }

        // calculate the number of bytes back into the history buffer to read
        let offset = range.decode_variable_length(&mut self.bit_reader)?;
        if offset >= pos {
            return Err(LhaError::Decompress(DecompressionError::HistoryDistanceOverflow))
        }

        // start copying from the ring buffer
        self.copy_from_history(target, offset, count);
        Ok(())
    }

    /// Read the index into the byte decode table, using the byte_decode_tree
    /// set at the start of the stream.
    ///
    /// The returned range entry maximum decoded value is 255.
    fn read_byte_decode_range(&mut self) -> LhaResult<VarLenEntry, R> {
        // Table used to decode byte values.
        type E = VarLenEntry;
        const BYTE_RANGES: [VarLenEntry;6] = [
            E::new(   0, 4 ),  //   0 + (1 << 4) - 1 = 15
            E::new(  16, 4 ),  //  16 + (1 << 4) - 1 = 31
            E::new(  32, 5 ),  //  32 + (1 << 5) - 1 = 63
            E::new(  64, 6 ),  //  64 + (1 << 6) - 1 = 127
            E::new( 128, 6 ),  // 128 + (1 << 6) - 1 = 191
            E::new( 192, 6 ),  // 192 + (1 << 6) - 1 = 255
        ];

        if self.byte_decode_tree[0] == 0 {
            return Ok(BYTE_RANGES[0]);
        }

        // Walk down the tree, reading a bit at each node to determine
        // which path to take.
        let mut tree = &self.byte_decode_tree[..];
        loop {
            let node = tree[0];
            let child = usize::from(if self.bit_reader.read_bit()? {
                node
            }
            else {
                node >> 4
            }) & 0x0f;
            // Reached a leaf node?
            match child {
                10.. => break Ok(BYTE_RANGES[child - 10]),
                i => {
                    assert!(i < tree.len());
                    tree = &tree[i..];
                }
            }
        }
    }

    /// Read a single byte value from the input stream
    fn read_byte(&mut self) -> LhaResult<u8, R> {
        // Read the index into the byte_ranges table to use.
        let range = self.read_byte_decode_range()?;
        debug_assert!(u32::from(range.offs) + (1 << range.bits) - 1 <= u32::from(u8::MAX));
        // Decode value using byte_ranges table. This is actually
        // a distance to walk along the history linked list - it
        // is static huffman encoding, so that recently used byte
        // values use fewer bits.
        let offset = range.decode_variable_length(&mut self.bit_reader)?;
        debug_assert!(offset <= u16::from(u8::MAX));
        // Walk through the history linked list to get the actual value.
        Ok(self.history_list.find_in_history_list(offset as u8))
    }

    /// Read the length of a block of bytes.
    ///
    /// The returned value is in the range: 1..=216.
    fn read_byte_block_count(&mut self) -> LhaResult<u8, R> {
        // Simon Howard:
        // This is a form of static huffman coding, where smaller
        // lengths are encoded using shorter bit sequences.

        // .2 <3 (+1)
        //    3:
        //       .3 <7 (+4)
        //          7:
        //             .4 <14 (+11)
        //                14: .6 (+25)
        //                15: .7 (+89)

        // value in the range 1..=3?
        let x: u8 = self.bit_reader.read_bits(2)?;
        if x < 3 {
            return Ok(x + 1)
        }

        // value in the range 4..=10?
        let x: u8 = self.bit_reader.read_bits(3)?;
        if x < 7 {
            return Ok(x + 4)
        }

        let (bits, offset) = match self.bit_reader.read_bits(4)? {
            // value in the range 89..=216
            15 => (7, 89),
            // value in the range 25..=88:
            14 => (6, 25),
            // value in the range 11..=25?
            x  => return Ok(x + 11), // x < 14
        };
        self.bit_reader.read_bits(bits).map(|x: u8| x + offset)
    }

    /// Read a block of bytes from the input stream.
    fn read_next_byte_block<'a, I: ExactSizeIterator<Item=&'a mut u8>>(
            &mut self,
            target: I
        ) -> LhaResult<(), R>
    {
        // How many bytes to decode?
        let block_len = self.read_byte_block_count()?;
        debug_assert!((1..=MAX_BYTE_BLOCK_LEN).contains(&block_len));

        // Simon Howard:
        // Because this is a block of bytes, it can be assumed that the
        // block ended for a copy command. The one exception is that if
        // the maximum block length was reached, the block may have
        // ended just because it could not be any larger.
        let copy_next = block_len < MAX_BYTE_BLOCK_LEN;

        self.read_byte_block(target, block_len, copy_next)
    }
}

impl<R: Read> Decoder<R> for Pm1Decoder<R> where R::Error: core::error::Error {
    type Error = R::Error;

    fn into_inner(self) -> R {
        self.bit_reader.into_inner().0
    }

    fn get_ref(&self) -> &R {
        &self.bit_reader.get_ref().0
    }

    fn get_mut(&mut self) -> &mut R {
        &mut self.bit_reader.get_mut().0
    }

    fn fill_buffer(&mut self, buf: &mut[u8]) -> LhaResult<(), R> {
        // read the header if start of input stream
        if !self.byte_decode_tree_ready {
            self.read_start_header()?;
        }
        let mut target = buf.iter_mut();

        // continue previous operation?
        if let Some(progress) = self.progress {
            match progress {
                Progress::Read { count, copy_next } => {
                    self.read_byte_block(&mut target, count.get(), copy_next)?
                }
                Progress::Copy { offset, count } => {
                    self.copy_from_history(&mut target, offset, count.get());
                }
            }
        }

        while target.len() > 0 {
            // read what the type of command this is
            if self.bit_reader.read_bit()? {
                self.read_next_byte_block(&mut target)?;
            }
            else {
                self.read_copy_command(&mut target)?;
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
    fn pmarc1_works() {
        println!("Pm1Decoder<Empty> {}", size_of::<Pm1Decoder<io::Empty>>());
        println!("Pm1Decoder<File> {}", size_of::<Pm1Decoder<fs::File>>());
        println!("Progress {}", size_of::<Progress>());
        println!("RingArrayBuf<RING_BUFFER_SIZE> {}", size_of::<RingArrayBuf<RING_BUFFER_SIZE>>());
        let mut data: &[u8] = &[];
        let mut decoder = Pm1Decoder::new(&mut data);
        assert_eq!(decoder.get_ref(), &&mut &[]);
        assert_eq!(decoder.get_mut().read_all(&mut []).unwrap(), 0);
    }

    #[test]
    #[ignore = "long tests"]
    fn pmarc1_long_tests() {
        use rand::RngReader;
        let mut rng = rand::rng();
        let mut decoder = Pm1Decoder::new(RngReader(&mut rng));
        let mut buf = Vec::new();
        buf.resize(1024, 0);
        let mut n = 0usize;
        let start = Instant::now();
        let limit = Duration::from_secs(59);
        let mut errors = 0usize;
        while start.elapsed() <= limit {
            n += 1;
            for i in 1..=1024 {
                if let Err(err) = decoder.fill_buffer(&mut buf[0..i]) {
                    errors += 1;
                    assert!(matches!(err, LhaError::Decompress(DecompressionError::HistoryDistanceOverflow)));
                }
            }
        }
        println!("-pm1- iterations: {} errors: {} {:.2}%",
                    n, errors, (errors as f64 / n as f64) * 100.0);
    }
}
