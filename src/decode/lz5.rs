use core::slice;
use core::num::NonZeroU16;
use std::io::{self, Read};
use crate::decode::Decoder;
use crate::ringbuf::*;

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
    pub fn new(reader: R) -> Lz5Decoder<R> {
        let mut ringbuf = Box::new(RingArrayBuf::default());

        // fill 13 times with each byte value (3328)
        for i in 0..=255 {
            for _ in 0..13 {
                ringbuf.push(i);
            }
        }
        // 256 ascending values (3584)
        for i in 0..=255 {
            ringbuf.push(i);
        }
        // 256 descending values (3840)
        for i in (0..=255).rev() {
            ringbuf.push(i);
        }
        // 128 zeroes (3968)
        for _ in 0..128 {
            ringbuf.push(0);
        }
        // leave a gap of 110 default spaces (4078)
        ringbuf.set_cursor(START_OFFSET);
        // a margin of zeroes (4096)
        while ringbuf.cursor() != 0 {
            ringbuf.push(0);
        }
        // set the start offset
        ringbuf.set_cursor(START_OFFSET);

        Lz5Decoder {
            reader,
            ringbuf,
            bitmap: 1,
            copy_progress: None
        }
    }

    fn copy_from_history<'a, I: Iterator<Item=&'a mut u8> + ExactSizeIterator>(
            &mut self,
            target: I,
            pos: usize,
            count: usize
        ) -> io::Result<()>
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

impl<R: Read> Decoder<R> for Lz5Decoder<R> {
    fn into_inner(self) -> R {
        self.reader
    }

    fn fill_buffer(&mut self, buf: &mut[u8]) -> io::Result<()> {
        let buflen = buf.len();
        let mut target = buf.iter_mut();
        if let Some((pos, count)) = self.copy_progress {
            self.copy_from_history(&mut target,
                                   pos as usize,
                                   count.get() as usize)?;
        }

        let mut bitmap = self.bitmap;

        while let Some(dst) = target.next() {
            if bitmap == 1 {
                let mut byte = 0u8;
                self.reader.read_exact(slice::from_mut(&mut byte))?;
                bitmap = byte as u16 | 0x0100;
            }

            if bitmap & 1 == 1 {
                let mut value = 0u8;
                self.reader.read_exact(slice::from_mut(&mut value))?;
                *dst = value;
                self.ringbuf.push(value);
            }
            else {
                let mut cmd = [0u8;2];
                self.reader.read_exact(&mut cmd)?;
                let [lo, hi] = cmd;
                let pos = (((hi & 0xf0) as usize) << 4) | lo as usize;
                let count = (hi & 0x0f) as usize;
                let index = buflen - target.len() - 1;
                target = buf[index..].iter_mut();
                self.copy_from_history(&mut target, pos, count + 3)?;
            }

            bitmap >>= 1;
        }
        self.bitmap = bitmap;
        Ok(())
    }
}
