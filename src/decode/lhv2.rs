use core::num::NonZeroU32;
#[cfg(not(feature = "std"))]
use alloc::boxed::Box;
use crate::error::{LhaResult, LhaError};
use crate::stub_io::Read;
use crate::bitstream::*;
use crate::statictree::*;
use crate::ringbuf::*;

use super::Decoder;

const NUM_COMMANDS: usize = 510;
const NUM_TEMP_CODELEN: usize = 20;
/// The maximum number of allowed [`LhaDecoderConfig::HISTORY_BITS`].
pub const MAX_HISTORY_BITS: usize = 24;

/// LHArc version 2 configuration for [`LhaV2Decoder`].
pub trait LhaDecoderConfig {
    /// A ring buffer object of size equal to 2 to the power of ([`Self::HISTORY_BITS`] - 1).
    type RingBuffer: RingBuffer;
    /// The code lengths table size for building the offset tree.
    ///
    /// The value of this number is determining the maximum size of the
    /// history sliding window. E.g for 8192 byte sliding window set this to 14.
    ///
    /// This has to be no greater than [`MAX_HISTORY_BITS`].
    const HISTORY_BITS: u32;
    /// This number of bits is read to determine the size of the actual offset
    /// code length table.
    ///
    /// This is currently limited to be between 1 and 5 inclusive.
    const OFFSET_BITS: u32;
}

/// A generic decoder for LHArc version 2 compression methods.
#[derive(Debug)]
pub struct LhaV2Decoder<C: LhaDecoderConfig, R> {
    bit_reader: BitStream<R>,
    command_tree: HuffTree,
    offset_tree: HuffTree,
    remaining_commands: u16,
    copy_progress: Option<(u32, NonZeroU32)>,
    ringbuf: Box<C::RingBuffer>,
}

macro_rules! impl_lhav2_decoder {
    ($cfg_name:ident, HISTORY_BITS=$history_bits:literal, OFFSET_BITS=$offset_bits:literal) => {
        impl LhaDecoderConfig for $cfg_name {
            type RingBuffer = RingArrayBuf<{1u32.strict_shl($history_bits - 1) as usize}>;
            const HISTORY_BITS: u32 = $history_bits;
            const OFFSET_BITS: u32 = $offset_bits;
        }
    };
}

#[derive(Debug)]
pub struct Lh5DecoderCfg;
#[derive(Debug)]
pub struct Lh7DecoderCfg;
#[cfg(feature = "lhx")]
#[cfg_attr(docsrs, doc(cfg(feature = "lhx")))]
#[derive(Debug)]
pub struct LhxDecoderCfg;

impl_lhav2_decoder!(Lh5DecoderCfg, HISTORY_BITS=14, OFFSET_BITS=4);
impl_lhav2_decoder!(Lh7DecoderCfg, HISTORY_BITS=17, OFFSET_BITS=5);
#[cfg(feature = "lhx")]
impl_lhav2_decoder!(LhxDecoderCfg, HISTORY_BITS=20, OFFSET_BITS=5);

/// A decoder for `-lh4-` and `-lh5-` compression methods.
pub type Lh5Decoder<R> = LhaV2Decoder<Lh5DecoderCfg, R>;
/// A decoder for `-lh6-` and `-lh7-` compression methods.
pub type Lh7Decoder<R> = LhaV2Decoder<Lh7DecoderCfg, R>;
/// A decoder for `-lhx-` compression methods.
#[cfg(feature = "lhx")]
#[cfg_attr(docsrs, doc(cfg(feature = "lhx")))]
pub type LhxDecoder<R> = LhaV2Decoder<LhxDecoderCfg, R>;

impl<C: LhaDecoderConfig, R: Read> LhaV2Decoder<C, R> {
    pub fn new(rd: R) -> LhaV2Decoder<C, R> {
        assert_eq!(<C::RingBuffer as RingBuffer>::BUFFER_SIZE, const { 1 << (C::HISTORY_BITS - 1) });
        assert!((1..=5).contains(&C::OFFSET_BITS));
        assert!(C::HISTORY_BITS as usize <= MAX_HISTORY_BITS);
        let bit_reader = BitStream::new(rd);
        let ringbuf = Default::default();
        let command_tree = HuffTree::with_leaf_capacity(NUM_COMMANDS);
        let offset_tree = HuffTree::with_leaf_capacity(NUM_TEMP_CODELEN);
        LhaV2Decoder {
            bit_reader,
            ringbuf,
            command_tree,
            offset_tree,
            remaining_commands: 0,
            copy_progress: None
        }
    }

    // reads code length value, usually 0..=7 but might be higher
    fn read_code_length(&mut self) -> LhaResult<u8, R> {
        let mut len: u8 = self.bit_reader.read_bits(3)?;
        if len == 7 {
            while self.bit_reader.read_bit()? {
                len = len.checked_add(1).ok_or_else(||
                    LhaError::Decompress("code length overflow"))?;
            }
        }
        Ok(len)
    }

    // skip_range: 0, 1 or 2
    fn read_code_skip(&mut self, skip_range: u16) -> LhaResult<usize, R> {
        let (bits, increment) = match skip_range {
            0 => return Ok(1),
            1 => (4, 3), // 3..=18
            _ => (9, 20), // 20..=531
        };
        self.bit_reader.read_bits(bits).map(|skip: usize| skip + increment)
    }

    fn read_temp_tree(&mut self) -> LhaResult<(), R> {
        let mut code_lengths = [0u8; NUM_TEMP_CODELEN];

        // number of codes to read
        let num_codes: usize = self.bit_reader.read_bits(5)?;
        // println!("num codes: {:?}", num_codes);

        // single code only
        if num_codes == 0 {
            let code = self.bit_reader.read_bits(5)?;
            self.offset_tree.set_single(code);
            return Ok(());
        }

        if num_codes > NUM_TEMP_CODELEN {
            return Err(LhaError::Decompress("temporary codelen table has invalid size"))
        }

        // read actual lengths
        for p in code_lengths[0..num_codes.min(3)].iter_mut() {
            *p = self.read_code_length()?;
            // println!("length: {:?}", *p);
        }
        // 2-bit skip value follows
        let skip: usize = self.bit_reader.read_bits(2)?;
        // println!("skip: {:?}", skip);

        if 3 + skip > num_codes {
            return Err(LhaError::Decompress("temporary codelen table has invalid size"))
        }

        for p in code_lengths[3 + skip..num_codes].iter_mut() {
            *p = self.read_code_length()?;
            // println!("length: {:?}", *p);
        }

        self.offset_tree.build_tree(&code_lengths[0..num_codes])
            .map_err(LhaError::Decompress)?;
        Ok(())
    }

    fn read_command_tree(&mut self) -> LhaResult<(), R> {
        let mut code_lengths = [0u8; NUM_COMMANDS];

        // number of codes to read
        let num_codes: usize = self.bit_reader.read_bits(9)?;
        // println!("num codes: {:?}", num_codes);

        // single code only
        if num_codes == 0 {
            let code = self.bit_reader.read_bits(9)?;
            if usize::from(code) >= NUM_COMMANDS {
                return Err(LhaError::Decompress("invalid single command"))
            }
            self.command_tree.set_single(code);
            return Ok(());
        }

        if num_codes > NUM_COMMANDS {
            return Err(LhaError::Decompress("commands codelen table has invalid size"))
        }

        let mut index = 0;
        'outer: while index < num_codes {
            for (n, p) in code_lengths[index..num_codes].iter_mut().enumerate() {
                match self.offset_tree.read_entry(&mut self.bit_reader)? {
                    skip_range @ 0..=2 => {
                        let skip_count = self.read_code_skip(skip_range)?;
                        // println!("n: {} skip: {}", n + index, skip_count);
                        index += n + skip_count;
                        continue 'outer;
                    }
                    code => {
                        *p = (code - 2) as u8;
                        // println!("n: {} value: {}", index + n, *p);
                    }
                }
            }
            break;
        }

        self.command_tree.build_tree(&code_lengths[0..num_codes])
            .map_err(LhaError::Decompress)?;
        Ok(())
    }

    fn read_offset_tree(&mut self) -> LhaResult<(), R> {
        let mut code_lengths = [0u8; MAX_HISTORY_BITS];

        // number of codes to read
        let num_codes: usize = self.bit_reader.read_bits(C::OFFSET_BITS)?;
        // println!("num codes: {} bits: {}", num_codes, C::OFFSET_BITS);

        // single code only
        if num_codes == 0 {
            let code = self.bit_reader.read_bits(C::OFFSET_BITS)?;
            if u32::from(code) >= C::HISTORY_BITS {
                return Err(LhaError::Decompress("invalid single offset"))
            }
            self.offset_tree.set_single(code);
            return Ok(());
        }

        if num_codes > C::HISTORY_BITS as usize {
            return Err(LhaError::Decompress("offset codelen table has invalid size"))
        }

        // read actual lengths
        for p in code_lengths[0..num_codes].iter_mut() {
            *p = self.read_code_length()?;
            // println!("length: {}", *p);
        }

        self.offset_tree.build_tree(&code_lengths[0..num_codes])
            .map_err(LhaError::Decompress)?;
        Ok(())
    }

    fn begin_new_block(&mut self) -> LhaResult<(), R> {
        let remaining_commands = self.bit_reader.read_bits(16)?;
        self.read_temp_tree()?;
        self.read_command_tree()?;
        self.read_offset_tree()?;
        self.remaining_commands = remaining_commands;
        Ok(())
    }

    #[inline]
    fn read_command(&mut self) -> LhaResult<u16, R> {
        self.command_tree.read_entry(&mut self.bit_reader)
    }

    #[inline]
    fn read_offset(&mut self) -> LhaResult<u32, R> {
        match self.offset_tree.read_entry(&mut self.bit_reader)?.into() {
        //   bits => 0 ->    0
        //   bits => 1 ->    1
            res @ 0..=1 => Ok(res),
        //   bits => 2 ->   1x
        //   bits => 3 ->  1xx
        //   bits => 4 -> 1xxx
            bits => {
                let res: u32 = self.bit_reader.read_bits(bits - 1)?;
                Ok(res | (1 << (bits - 1)))
            }
        }
    }

    fn copy_from_history<'a, I: Iterator<Item=&'a mut u8> + ExactSizeIterator>(
            &mut self,
            target: I,
            offset: usize,
            count: usize
        )
    {
        let history_iter = self.ringbuf.iter_from_offset(offset);
        let count_after = count - target.len().min(count);
        for (t, s) in target.zip(history_iter).take(count) {
            *t = s;
        }
        self.copy_progress = NonZeroU32::new(count_after as u32)
                             .map(|count| (offset as u32, count));
    }

}

impl<C: LhaDecoderConfig, R: Read> Decoder<R> for LhaV2Decoder<C, R>
    where R::Error: core::error::Error
{
    type Error = R::Error;

    fn into_inner(self) -> R {
        self.bit_reader.into_inner()
    }

    fn fill_buffer(&mut self, buf: &mut[u8]) -> LhaResult<(), R> {
        let buflen = buf.len();
        let mut target = buf.iter_mut();
        if let Some((offset, count)) = self.copy_progress {
            self.copy_from_history(&mut target,
                                   offset as usize,
                                   count.get() as usize);
        }

        while let Some(dst) = target.next() {
            while self.remaining_commands == 0 {
                self.begin_new_block()?;
            }

            self.remaining_commands -= 1;

            match self.read_command()? {
                code @ 0..=0xff => {
                    let value = code as u8;
                    *dst = value;
                    self.ringbuf.push(value);
                }
                count => {
                    let offset = self.read_offset()?;
                    let index = buflen - target.len() - 1;
                    target = buf[index..].iter_mut();
                    self.copy_from_history(&mut target,
                                           offset as usize,
                                           (count - 0x100 + 3).into());
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
    use super::super::DecoderAny;

    #[test]
    fn lhav2_works() {
        println!("DecoderAny<Empty> {}", size_of::<DecoderAny<io::Empty>>());
        println!("DecoderAny<fs::File> {}", size_of::<DecoderAny<fs::File>>());
        println!("Lh7Decoder<Empty> {}", size_of::<Lh7Decoder<io::Empty>>());
        println!("Lh7Decoder<File> {}", size_of::<Lh7Decoder<fs::File>>());
        println!("BitStream<File> {}", size_of::<BitStream<fs::File>>());
        println!("HuffTree {}", size_of::<HuffTree>());
        println!("HuffTree offset tree: {}", size_of::<[TreeEntry;NUM_TEMP_CODELEN * 2]>());
        println!("HuffTree command tree: {}", size_of::<[TreeEntry;NUM_COMMANDS * 2]>());
        println!("Option<(u32, NonZeroU32)> {}", size_of::<Option<(u32, NonZeroU32)>>());
        println!("Box<C::RingBuffer> {}", size_of::<Box<<Lh7DecoderCfg as LhaDecoderConfig>::RingBuffer>>());
        println!("Lh5::RingBuffer {}", size_of::<<Lh5DecoderCfg as LhaDecoderConfig>::RingBuffer>());
        println!("Lh7::RingBuffer {}", size_of::<<Lh7DecoderCfg as LhaDecoderConfig>::RingBuffer>());
        #[cfg(feature = "lhx")]
        println!("Lhx::RingBuffer {}", size_of::<<LhxDecoderCfg as LhaDecoderConfig>::RingBuffer>());
        let _ = Lh7Decoder::new(io::empty());
        let _ = Lh5Decoder::new(io::empty());
        #[cfg(feature = "lhx")]
        let _ = LhxDecoder::new(io::empty());
    }

    #[test]
    #[ignore = "long tests"]
    fn lhav2_long_tests() {
        use rand::{Rng, RngExt, RngReader, seq::SliceRandom};

        // build a random tree lengths with an upper num of values and max depth
        fn build_random_lengths(
                max_values: usize,
                mut max_depth: u8,
                final_size: usize,
                rng: &mut impl Rng,
                out: &mut Vec<u8>
            )
        {
            out.clear();
            let mut max_leaves = 2usize;
            for level in 1..max_depth {
                let n = out.len();
                let remaining = max_values - n;
                let num_leaves;
                if let Some(margin) = (max_leaves * 2).checked_sub(remaining)  {
                    if remaining <= max_leaves {
                        max_depth = level;
                        break
                    }
                    num_leaves = margin;
                }
                else {
                    num_leaves = rng.random_range(0..max_leaves);
                };
                max_leaves = (max_leaves - num_leaves) * 2;
                out.resize(n + num_leaves, level);
            }
            out.resize(out.len() + max_leaves, max_depth);
            assert!(final_size >= out.len(), "final_size: {} < out.len: {}", final_size, out.len());
            out.resize(final_size, 0);
            out.shuffle(rng);
        }

        let mut rng = rand::rng();
        let mut decoder = Lh5Decoder::new(RngReader(&mut rng));
        let mut rng = rand::rng();
        let mut buf = Vec::new();
        let mut code_lengths = Vec::new();
        buf.resize(1024, 0);
        let mut max_temp = 0;
        let mut max_command = 0;
        let mut max_offset = 0;
        let mut i = 0usize;
        let start = Instant::now();
        let limit = Duration::from_secs(59);
        while start.elapsed() <= limit {
            i += 1;
            // let mut err = 0u64;
            let mut max = 0;
            for _ in 0..1000 {
                if decoder.read_temp_tree().is_err() {
                    // err += 1;
                }
                else {
                    max = max.max(decoder.offset_tree.len());
                }
            }
            max_temp = max_temp.max(max);
            // println!("-lh5-: read_temp_tree: {} errors: {}/1000", max, err);
            match rng.random_range(1..=NUM_TEMP_CODELEN) {
                1 => {
                    decoder.offset_tree.set_single(rng.random_range(0..=31));
                    // println!("-lh5-: temp single: {:?}", decoder.offset_tree.inspect()[0]);
                }
                max => {
                    build_random_lengths(max, 10, NUM_TEMP_CODELEN, &mut rng, &mut code_lengths);
                    decoder.offset_tree.build_tree(&code_lengths).unwrap();
                    // println!("-lh5-: temp max: {} \n{}", max, decoder.offset_tree);
                }
            }

            // let mut err = 0u64;
            let mut max = 0;
            for _ in 0..1000 {
                if decoder.read_command_tree().is_err() {
                    // err += 1;
                }
                else {
                    max = max.max(decoder.command_tree.len());
                }
            }
            max_command = max_command.max(max);
            // println!("-lh5-: read_command_tree: {} errors: {}/1000", max, err);
            match rng.random_range(1..=NUM_COMMANDS) {
                1 => {
                    decoder.command_tree.set_single(rng.random_range(0..NUM_COMMANDS as u16));
                    // println!("-lh5-: command single: {:?}", decoder.command_tree.inspect()[0]);
                }
                max => {
                    build_random_lengths(max, (NUM_TEMP_CODELEN - 1) as u8, NUM_COMMANDS, &mut rng, &mut code_lengths);
                    decoder.command_tree.build_tree(&code_lengths).unwrap();
                    // println!("-lh5-: command max: {} \n{}", max, decoder.command_tree);
                }
            }

            // let mut err = 0u64;
            let mut max = 0;
            for _ in 0..1000 {
                if decoder.read_offset_tree().is_err() {
                    // err += 1;
                }
                else {
                    max = max.max(decoder.offset_tree.len());
                }
            }
            max_offset = max_offset.max(max);
            // println!("-lh5-: read_offset_tree: {} errors: {}/1000", max, err);
            match rng.random_range(1..=Lh5DecoderCfg::HISTORY_BITS as usize) {
                1 => {
                    decoder.offset_tree.set_single(rng.random_range(0..Lh5DecoderCfg::HISTORY_BITS as u16));
                    // println!("-lh5-: offset single: {:?}", decoder.offset_tree.inspect()[0]);
                }
                max => {
                    build_random_lengths(max, 10, Lh5DecoderCfg::HISTORY_BITS as usize, &mut rng, &mut code_lengths);
                    decoder.offset_tree.build_tree(&code_lengths).unwrap();
                    // println!("-lh5-: offset max: {} \n{}", max, decoder.offset_tree);
                }
            }

            decoder.remaining_commands = u16::MAX;

            for n in 1..=1024 {
                let len = n.min(decoder.remaining_commands as usize);
                decoder.fill_buffer(&mut buf[0..len]).unwrap();
                if decoder.remaining_commands == 0 {
                    break
                }
            }
        }
        println!("-lh5- read_temp_tree: {}", max_temp);
        println!("-lh5- read_command_tree: {}", max_command);
        println!("-lh5- read_offset_tree: {}", max_offset);
        println!("-lh5- iterations: {}", i);
    }
}
