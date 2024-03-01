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

pub trait LhaDecoderConfig {
    type RingBuffer: RingBuffer;
    const HISTORY_BITS: u32;
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
        #[derive(Debug)]
        pub struct $cfg_name;

        impl LhaDecoderConfig for $cfg_name {
            type RingBuffer = RingArrayBuf<{1 << $history_bits - 1}>;
            const HISTORY_BITS: u32 = $history_bits;
            const OFFSET_BITS: u32 = $offset_bits;
        }
    };
}

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
pub type LhxDecoder<R> = LhaV2Decoder<LhxDecoderCfg, R>;


impl<C: LhaDecoderConfig, R: Read> LhaV2Decoder<C, R> {
    pub fn new(rd: R) -> LhaV2Decoder<C, R> {
        let bit_reader = BitStream::new(rd);
        let ringbuf = Default::default();
        let command_tree = HuffTree::with_capacity(NUM_COMMANDS * 2);
        let offset_tree = HuffTree::with_capacity(NUM_TEMP_CODELEN * 2);
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
        debug_assert!(NUM_TEMP_CODELEN >= C::HISTORY_BITS as usize);
        let mut code_lengths = [0u8; NUM_TEMP_CODELEN];

        // number of codes to read
        let num_codes: usize = self.bit_reader.read_bits(C::OFFSET_BITS)?;
        // println!("num codes: {} bits: {}", num_codes, C::OFFSET_BITS);

        // single code only
        if num_codes == 0 {
            let code = self.bit_reader.read_bits(C::OFFSET_BITS)?;
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
        self.remaining_commands = self.bit_reader.read_bits(16)?;
        self.read_temp_tree()?;
        self.read_command_tree()?;
        self.read_offset_tree()
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
        ) -> LhaResult<(), R>
    {
        let history_iter = self.ringbuf.iter_from_offset(offset);
        let count_after = count - target.len().min(count);
        for (t, s) in target.zip(history_iter).take(count) {
            *t = s;
        }
        self.copy_progress = NonZeroU32::new(count_after as u32)
                             .map(|count| (offset as u32, count));
        Ok(())
    }

}

impl<C: LhaDecoderConfig, R: Read> Decoder<R> for LhaV2Decoder<C, R>
    where R::Error: core::fmt::Debug
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
                                   count.get() as usize)?;
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
                                           (count - 0x100 + 3).into())?;
                }
            }
        }
        Ok(())
    }
}

#[cfg(feature = "std")]
#[cfg(test)]
mod tests {
    use super::*;
    use super::super::DecoderAny;
    use std::fs;
    use std::io;

    #[test]
    fn lhav2_works() {
        println!("DecoderAny<Empty> {}", core::mem::size_of::<DecoderAny<io::Empty>>());
        println!("DecoderAny<fs::File> {}", core::mem::size_of::<DecoderAny<fs::File>>());
        println!("Lh7Decoder<Empty> {}", core::mem::size_of::<Lh7Decoder<io::Empty>>());
        println!("Lh7Decoder<File> {}", core::mem::size_of::<Lh7Decoder<fs::File>>());
        println!("BitStream<File> {}", core::mem::size_of::<BitStream<fs::File>>());
        println!("HuffTree {}", core::mem::size_of::<HuffTree>());
        println!("Option<(u32, NonZeroU32)> {}", core::mem::size_of::<Option<(u32, NonZeroU32)>>());
        println!("Box<C::RingBuffer> {}", core::mem::size_of::<Box<<Lh7DecoderCfg as LhaDecoderConfig>::RingBuffer>>());
    }
}
