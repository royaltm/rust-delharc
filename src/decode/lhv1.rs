use core::num::NonZeroU16;
use std::io::{self, Read};
use crate::decode::Decoder;
use crate::ringbuf::*;
use crate::bitstream::*;

pub mod dyntree;
use dyntree::*;

const RING_BUFFER_SIZE: usize = 4096;

/// A decoder for `-lh1-` compression method.
#[derive(Debug)]
pub struct Lh1Decoder<R> {
    bit_reader: BitStream<R>,
    command_tree: Box<DynHuffTree>,
    copy_progress: Option<(u16, NonZeroU16)>,
    ringbuf: Box<RingArrayBuf<[u8;RING_BUFFER_SIZE]>>,
}

impl<R: Read> Lh1Decoder<R> {
    pub fn new(rd: R) -> Lh1Decoder<R> {
        let bit_reader = BitStream::new(rd);
        let ringbuf = Default::default();
        let command_tree = Box::new(DynHuffTree::new());
        Lh1Decoder {
            bit_reader,
            ringbuf,
            command_tree,
            copy_progress: None
        }
    }

    #[inline]
    fn read_command(&mut self) -> io::Result<u16> {
        self.command_tree.read_entry(&mut self.bit_reader)
    }

    #[inline]
    fn read_offset(&mut self) -> io::Result<u16> {
        let bits9 = self.bit_reader.read_bits(9)?;
        let (mut offset, bits) = decode_offset(bits9);
        offset |= self.bit_reader.read_bits::<u16>(bits)?;
        Ok(offset)
    }

    fn copy_from_history<'a, I: Iterator<Item=&'a mut u8> + ExactSizeIterator>(
            &mut self,
            target: I,
            offset: usize,
            count: usize
        ) -> io::Result<()>
    {
        let history_iter = self.ringbuf.iter_from_offset(offset);
        let count_after = count - target.len().min(count);
        for (t, s) in target.zip(history_iter).take(count) {
            *t = s;
        }
        self.copy_progress = NonZeroU16::new(count_after as u16)
                             .map(|count| (offset as u16, count));
        Ok(())
    }
}

impl<R: Read> Decoder<R> for Lh1Decoder<R> {
    fn into_inner(self) -> R {
        self.bit_reader.into_inner()
    }

    fn fill_buffer(&mut self, buf: &mut[u8]) -> io::Result<()> {
        let buflen = buf.len();
        let mut target = buf.iter_mut();
        if let Some((offset, count)) = self.copy_progress {
            self.copy_from_history(&mut target,
                                   offset as usize,
                                   count.get() as usize)?;
        }

        while let Some(dst) = target.next() {
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

/// Returns (incomplete offset, additional bits to read)
#[inline(always)]
fn decode_offset(bits9: u16) -> (u16, u32) {
    match bits9 & 0b111100000 {
       /* 000xxxxxx -> 000000 xxxxxx */
        0b000000000..=0b000111111 => (bits9, 0),
       /* 0010xxxxx -> 000001 xxxxxy */
       /* 0100xxxxx -> 000011 xxxxxy */
        0b001000000..=0b010011111 => ((bits9 - 0b000100000) << 1, 1),
       /* 01010xxxx -> 000100 xxxxyy */
       /* 10001xxxx -> 001011 xxxxyy */
        0b010100000..=0b100011111 => ((bits9 - 0b001100000) << 2, 2),
       /* 100100xxx -> 001100 xxxyyy */
       /* 101111xxx -> 010111 xxxyyy */
        0b100100000..=0b101111111 => ((bits9 - 0b011000000) << 3, 3),
       /* 1100000xx -> 011000 xxyyyy */
       /* 1110111xx -> 101111 xxyyyy */
        0b110000000..=0b111011111 => ((bits9 - 0b100100000) << 4, 4),
       /* 11110000x -> 110000 xyyyyy */
       /* 11111111x -> 111111 xyyyyy */
        0b111100000..=0b111111111 => ((bits9 - 0b110000000) << 5, 5),
        _ => unreachable!()
    }
}
