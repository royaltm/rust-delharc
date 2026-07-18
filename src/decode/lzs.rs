//! LArc -lzs- decoder
//!
//! Original C version: (c) 2011, 2012, Simon Howard lhasa/lib/lzs_decoder.c
//!
//! Rust version: (c) 2018-2026, Rafał Michalski
#[cfg(not(feature = "std"))]
use alloc::boxed::Box;
use core::num::NonZeroU16;
use bytemuck::allocation::zeroed_box;
use crate::{
    bitstream::*,
    decode::Decoder,
    error::LhaResult,
    ringbuf::*,
    stub_io::Read,
};
use super::unsafe_assert;

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
    /// Create a new decoder instance from the given data read stream
    pub fn new(rd: R) -> LzsDecoder<R> {
        let bit_reader = BitStream::new(rd);
        let mut ringbuf = zeroed_box::<RingArrayBuf<RING_BUFFER_SIZE>>();
        ringbuf.initialize(b' ');
        ringbuf.set_cursor(START_OFFSET);
        LzsDecoder {
            bit_reader,
            ringbuf,
            copy_progress: None
        }
    }

    /// Progressively copy data from history buffer
    fn copy_from_history<'a, I: ExactSizeIterator<Item=&'a mut u8>>(
            &mut self,
            target: I,
            pos: usize,
            count: usize
        )
    {
        let history_iter = self.ringbuf.iter_from_index(pos);
        let real_count = target.len().min(count);
        for (t, s) in target.zip(history_iter).take(real_count) {
            *t = s;
        }
        self.copy_progress = NonZeroU16::new((count - real_count) as u16)
                             .map(|count| ((pos + real_count) as u16, count));
    }
}

impl<R: Read> Decoder<R> for LzsDecoder<R> where R::Error: core::error::Error {
    type Error = R::Error;

    fn into_inner(self) -> R {
        self.bit_reader.into_inner()
    }

    fn get_ref(&self) -> &R {
        self.bit_reader.get_ref()
    }

    fn get_mut(&mut self) -> &mut R {
        self.bit_reader.get_mut()
    }

    fn fill_buffer(&mut self, buf: &mut[u8]) -> LhaResult<(), R> {
        let buflen = buf.len();
        let mut target = buf.iter_mut();
        if let Some((pos, count)) = self.copy_progress {
            self.copy_from_history(&mut target,
                                   pos as usize,
                                   count.get() as usize);
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
                // SAFETY: target.len() < buf.len() because target is an
                // iterator over buf which has yield at least one item
                unsafe_assert!(index < buf.len());
                target = buf[index..].iter_mut();
                self.copy_from_history(&mut target, pos, count + 2);
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
    fn lzs_works() {
        println!("LzsDecoder<Empty> {}", size_of::<LzsDecoder<io::Empty>>());
        println!("LzsDecoder<fs::File> {}", size_of::<LzsDecoder<fs::File>>());
        println!("RingArrayBuf<RING_BUFFER_SIZE> {}", size_of::<RingArrayBuf<RING_BUFFER_SIZE>>());
        let _ = LzsDecoder::new(io::empty());
    }

    #[test]
    #[ignore = "long tests"]
    fn lzs_long_tests() {
        use rand::RngReader;
        let mut rng = rand::rng();
        let mut decoder = LzsDecoder::new(RngReader(&mut rng));
        let mut buf = Vec::new();
        buf.resize(1024, 0);
        let mut n = 0usize;
        let start = Instant::now();
        let limit = Duration::from_secs(59);
        while start.elapsed() <= limit {
            n += 1;
            for i in 1..=1024 {
                decoder.fill_buffer(&mut buf[0..i]).unwrap()
            }
        }
        println!("-lzs- iterations: {}", n);
    }
}
