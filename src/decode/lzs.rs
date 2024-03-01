use core::num::NonZeroU16;
#[cfg(not(feature = "std"))]
use alloc::boxed::Box;
use crate::error::LhaResult;
use crate::stub_io::Read;
use crate::decode::Decoder;
use crate::ringbuf::*;
use crate::bitstream::*;

const RING_BUFFER_SIZE: usize = 2048;
const START_OFFSET: isize = -17;

/// A decoder for `-lzs-` compression method.
#[derive(Debug)]
pub struct LzsDecoder<R> {
    bit_reader: BitStream<R>,
    copy_progress: Option<(u16, NonZeroU16)>,
    ringbuf: Box<RingArrayBuf<RING_BUFFER_SIZE>>,
}

impl<R: Read> LzsDecoder<R> {
    pub fn new(rd: R) -> LzsDecoder<R> {
        let bit_reader = BitStream::new(rd);
        let mut ringbuf: Box<RingArrayBuf<RING_BUFFER_SIZE>> = Box::default();
        ringbuf.set_cursor(START_OFFSET);
        LzsDecoder {
            bit_reader,
            ringbuf,
            copy_progress: None
        }
    }

    fn copy_from_history<'a, I: Iterator<Item=&'a mut u8> + ExactSizeIterator>(
            &mut self,
            target: I,
            pos: usize,
            count: usize
        ) -> LhaResult<(), R>
    {
        let history_iter = self.ringbuf.iter_from_pos(pos);
        let real_count = target.len().min(count);
        for (t, s) in target.zip(history_iter).take(real_count) {
            *t = s;
        }
        self.copy_progress = NonZeroU16::new((count - real_count) as u16)
                             .map(|count| ((pos + real_count) as u16, count));
        Ok(())
    }
}

impl<R: Read> Decoder<R> for LzsDecoder<R> where R::Error: core::fmt::Debug {
    type Error = R::Error;

    fn into_inner(self) -> R {
        self.bit_reader.into_inner()
    }

    fn fill_buffer(&mut self, buf: &mut[u8]) -> LhaResult<(), R> {
        let buflen = buf.len();
        let mut target = buf.iter_mut();
        if let Some((pos, count)) = self.copy_progress {
            self.copy_from_history(&mut target,
                                   pos as usize,
                                   count.get() as usize)?;
        }

        while let Some(dst) = target.next() {
            if self.bit_reader.read_bit()? {
                let value = self.bit_reader.read_bits(8)?;
                *dst = value;
                self.ringbuf.push(value);
            }
            else {
                let pos = self.bit_reader.read_bits(11)?;
                let count: usize = self.bit_reader.read_bits(4)?;
                let index = buflen - target.len() - 1;
                target = buf[index..].iter_mut();
                self.copy_from_history(&mut target, pos, count + 2)?;
            }
        }
        Ok(())
    }
}
