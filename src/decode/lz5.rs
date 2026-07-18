#[cfg(not(feature = "std"))]
use alloc::boxed::Box;
use core::{num::NonZeroU16, slice};
use crate::{
    decode::Decoder,
    error::{LhaResult, LhaError},
    ringbuf::*,
    stub_io::Read,
};
use bytemuck::allocation::zeroed_box;

const RING_BUFFER_SIZE: usize = 4096;
const START_OFFSET: isize = -18;

/// A decoder for `-lz5-` compression method.
#[derive(Debug)]
pub struct Lz5Decoder<R> {
    reader: R,
    bitmap: u16,
    copy_progress: Option<(u16, NonZeroU16)>,
    ringbuf: Box<RingArrayBuf<RING_BUFFER_SIZE>>,
}

impl<R: Read> Lz5Decoder<R> {
    /// Create a new decoder instance from the given data read stream
    pub fn new(reader: R) -> Lz5Decoder<R> {
        let mut ringbuf = zeroed_box::<RingArrayBuf<RING_BUFFER_SIZE>>();
        ringbuf.initialize_with(|buffer| {
            assert_eq!(buffer.len(), RING_BUFFER_SIZE);
            // fill 13 times with each byte value (3328)
            for (chunk, i) in buffer.as_chunks_mut::<13>().0.iter_mut().zip(0..=255u8) {
                chunk.fill(i);
            }
            // 256 ascending values (3584)
            let offset = 256 * 13;
            for (p, i) in buffer[offset..].iter_mut().zip(0..=255u8) {
                *p = i;
            }
            // 256 descending values (3840)
            let offset = offset + 256;
            for (p, i) in buffer[offset..].iter_mut().zip((0..=255u8).rev()) {
                *p = i;
            }
            // 128 zeroes (3968)
            let offset = offset + 256;
            buffer[offset..offset + 128].fill(0);
            // leave a gap of 110 default spaces (4078)
            let offset = offset + 128;
            buffer[offset..offset + 110].fill(b' ');
            // a margin of zeroes (4096)
        });
        // set the start offset
        ringbuf.set_cursor(START_OFFSET);

        Lz5Decoder {
            reader,
            ringbuf,
            bitmap: 1,
            copy_progress: None
        }
    }

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

impl<R: Read> Decoder<R> for Lz5Decoder<R> where R::Error: core::error::Error {
    type Error = R::Error;

    fn into_inner(self) -> R {
        self.reader
    }

    fn get_ref(&self) -> &R {
        &self.reader
    }

    fn get_mut(&mut self) -> &mut R {
        &mut self.reader
    }

    fn fill_buffer(&mut self, buf: &mut[u8]) -> LhaResult<(), R> {
        let buflen = buf.len();
        let mut target = buf.iter_mut();
        if let Some((pos, count)) = self.copy_progress {
            self.copy_from_history(&mut target,
                                   pos as usize,
                                   count.get() as usize);
        }

        let mut bitmap = self.bitmap;

        while let Some(dst) = target.next() {
            if bitmap == 1 {
                let mut byte = 0u8;
                self.reader.read_exact(slice::from_mut(&mut byte))
                           .map_err(LhaError::Io)?;
                bitmap = byte as u16 | 0x0100;
            }

            if bitmap & 1 == 1 {
                let mut value = 0u8;
                self.reader.read_exact(slice::from_mut(&mut value))
                           .map_err(LhaError::Io)?;
                *dst = value;
                self.ringbuf.push(value);
            }
            else {
                let mut cmd = [0u8;2];
                self.reader.read_exact(&mut cmd)
                           .map_err(LhaError::Io)?;
                let [lo, hi] = cmd;
                let pos = (((hi & 0xf0) as usize) << 4) | lo as usize;
                let count = (hi & 0x0f) as usize;
                let index = buflen - target.len() - 1;
                target = buf[index..].iter_mut();
                self.copy_from_history(&mut target, pos, count + 3);
            }

            bitmap >>= 1;
        }
        self.bitmap = bitmap;
        Ok(())
    }
}


#[cfg(feature = "std")]
#[cfg(test)]
mod tests {
    use std::{io, fs, time::{Instant, Duration}};
    use super::*;

    #[test]
    fn lz5_works() {
        println!("Lz5Decoder<Empty> {}", size_of::<Lz5Decoder<io::Empty>>());
        println!("Lz5Decoder<fs::File> {}", size_of::<Lz5Decoder<fs::File>>());
        println!("RingArrayBuf<RING_BUFFER_SIZE> {}", size_of::<RingArrayBuf<RING_BUFFER_SIZE>>());
        let _ = Lz5Decoder::new(io::empty());
    }

    #[test]
    #[ignore = "long tests"]
    fn lz5_long_tests() {
        use rand::RngReader;
        let mut rng = rand::rng();
        let mut decoder = Lz5Decoder::new(RngReader(&mut rng));
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
        println!("-lz5- iterations: {}", n);
    }
}
