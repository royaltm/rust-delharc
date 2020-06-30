//! # Bit-stream tools.
use core::mem;
use std::io::{self, Read};

type BitBuf = usize;
const BITBUF_BYTESIZE: usize = mem::size_of::<BitBuf>();
const BITBUF_BITSIZE: u32 = (BITBUF_BYTESIZE * 8) as u32;

/// The trait is implemented for all the types that can receive bits using [BitRead::read_bits].
pub trait UBits: Copy {
    fn from_bits(bitbuf: BitBuf) -> Self;
}

/// This trait is being used to read single bits from data source.
pub trait BitRead {
    /// Reads the next single bit from the stream. `true` represents `1` and `false` represents `0`.
    fn read_bit(&mut self) -> io::Result<bool>;
    /// Reads the next `n` bits from the stream.
    ///
    /// For example reading 4 bits into the `u8` type will result in: `0b0000abcd` where
    /// `a`, `b`, `c`, `d` are consecutive bits that were read from source.
    ///
    /// Returns `0` if `n` is `0`.
    ///
    /// # Panics
    /// Panics if `n` exceed the bit capacity of `T`.
    fn read_bits<T: UBits>(&mut self, n: u32) -> io::Result<T>;
    /// Creates a "by reference" adaptor for this instance of `BitRead`.
    /// The returned adaptor also implements `BitRead` and will simply borrow this current reader.
    fn by_ref(&mut self) -> &mut Self {
        self
    }
}

/// A simple bit-stream reader, wrapped over a readable stream.
///
/// Bits are being read from an each consecutive byte, starting from its highest bit.
#[derive(Debug)]
pub struct BitStream<R> {
    inner: R,
    // x..x10..0
    bits_buf: BitBuf,
}

macro_rules! impl_ubits {
    ($ty:ty) => {
        impl UBits for $ty {
            #[inline(always)]
            fn from_bits(bitbuf: BitBuf) -> Self {
                bitbuf as $ty
            }
        }
    };
}

impl_ubits!(u8);
impl_ubits!(u16);
impl_ubits!(u32);
impl_ubits!(usize);

impl<R: Read> BitStream<R> {
    /// Creates a new `BitStream<R>`.
    pub fn new(inner: R) -> BitStream<R> {
        BitStream { inner, bits_buf: 1 << (BITBUF_BITSIZE - 1) }
    }
    /// Unwraps this `BitStream<R>`, returning the underlying reader.
    ///
    /// Note that any leftover data in the internal bit buffer is lost. Therefore, a following read from
    /// the underlying reader may lead to data loss.
    pub fn into_inner(self) -> R {
        self.inner
    }
    /// Gets a mutable reference to the underlying reader.
    ///
    /// It is inadvisable to directly read from the underlying reader.
    pub fn get_mut(&mut self) -> &mut R {
        &mut self.inner
    }
    /// Gets a reference to the underlying reader.
    ///
    /// It is inadvisable to directly read from the underlying reader.
    pub fn get_ref(&self) -> &R {
        &self.inner
    }

    #[inline]
    fn next_bits(&mut self, n: u32) -> io::Result<BitBuf> {
        debug_assert!(n != 0 && n <= BITBUF_BITSIZE);
        let have_bits = BITBUF_BITSIZE - self.bits_buf.trailing_zeros() - 1;
        let res = self.bits_buf >> BITBUF_BITSIZE - n;

        if n <= have_bits {
            self.bits_buf <<= n;
            return Ok(res)
        }

        let missing_bits = n - have_bits;
        let mut buf = [0u8;BITBUF_BYTESIZE];
        let bits_read = 8 * self.read_exact_or_to_end(&mut buf)? as u32;
        if bits_read < missing_bits {
            return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "some bits are missing from stream"))
        }
        let new_bits: BitBuf = BitBuf::from_be_bytes(buf);
        // clear trailing bits and merge
        let res = res & res - 1
                | new_bits >> BITBUF_BITSIZE - missing_bits;
        self.bits_buf = if missing_bits == BITBUF_BITSIZE {
            0
        }
        else {
            new_bits << missing_bits
        } | 1 << BITBUF_BITSIZE - 1 - (bits_read - missing_bits);
        Ok(res)
    }

    #[inline]
    fn read_exact_or_to_end(&mut self, mut buf: &mut[u8]) -> io::Result<usize> {
        let orig_len = buf.len();
        while !buf.is_empty() {
            match self.inner.read(buf) {
                Ok(0) => break,
                Ok(n) => buf = &mut buf[n..],
                Err(ref e) if e.kind() == io::ErrorKind::Interrupted => {}
                Err(e) => return Err(e),
            }
        }
        Ok(orig_len - buf.len())
    }
}

impl<R: BitRead> BitRead for &mut R {
    #[inline]
    fn read_bit(&mut self) -> io::Result<bool> {
        (*self).read_bit()
    }

    #[inline]
    fn read_bits<T: UBits>(&mut self, n: u32) -> io::Result<T> {
        (*self).read_bits(n)
    }
}

impl<R: Read> BitRead for BitStream<R> {
    #[inline]
    fn read_bit(&mut self) -> io::Result<bool> {
        self.next_bits(1).map(|bits| bits != 0)
    }

    fn read_bits<T: UBits>(&mut self, n: u32) -> io::Result<T> {
        match n {
            0 => Ok(0),
            n if n <= bitsize::<T>() => self.next_bits(n),
            _ => Err(io::Error::new(io::ErrorKind::InvalidInput, "too many bits requested"))
        }.map(T::from_bits)
    }
}

#[inline(always)]
const fn bitsize<T>() -> u32 {
    mem::size_of::<T>() as u32 * 8
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bit_stream_works() {
        assert_eq!(BITBUF_BYTESIZE, mem::size_of::<usize>());
        assert!(BITBUF_BITSIZE >= 32);
        assert_eq!(BITBUF_BITSIZE, BITBUF_BYTESIZE as u32 * 8);
        let mut somebits: &[u8] = &[];
        let mut brdr = BitStream::new(&mut somebits);
        assert_eq!(brdr.read_bit().unwrap_err().kind(), io::ErrorKind::UnexpectedEof);
        assert_eq!(brdr.read_bits::<usize>(BITBUF_BITSIZE).unwrap_err().kind(), io::ErrorKind::UnexpectedEof);
        let mut somebits: &[u8] = &[0];
        let mut brdr = BitStream::new(&mut somebits);
        for _ in 0..8 {
            assert_eq!(brdr.read_bit().unwrap(), false);
        }
        let mut somebits: &[u8] = &[!0];
        let mut brdr = BitStream::new(&mut somebits);
        for _ in 0..8 {
            assert_eq!(brdr.read_bit().unwrap(), true);
        }
        let mut somebits: &[u8] = &[0b01001100, 0b01110000, 0b11110000, 0b01111100, 0b00001111, 0b11000000, 0b01111111,
                                    0b00000000, 0b11111111, 0b00000000, 0b01111111, 0b11000000, 0b00001111, 0b11111100,
                                    0b00000000, 0b01111111, 0b11110000, 0b00000000, 0b11111111, 0b11110000, 0b00000000,
                                    0b01111111, 0b11111100, 0b00000000, 0b00001111, 0b11111111, 0b11000000, 0b00000000,
                                    0b01111111, 0b11111111];
        assert_eq!(brdr.read_bits::<usize>(0).unwrap(), 0);
        let mut brdr = BitStream::new(&mut somebits);
        for n in 1..16 {
            assert_eq!(brdr.read_bits::<u16>(n).unwrap(), 0);
            assert_eq!(brdr.read_bits::<u16>(n).unwrap(), (1 << n) - 1);
        }
        assert_eq!(brdr.read_bits::<usize>(0).unwrap(), 0);
        assert_eq!(brdr.read_bit().unwrap_err().kind(), io::ErrorKind::UnexpectedEof);

        let mut somebits: &[u8] = &[1,2,3,4,5,6,7,8];
        let mut brdr = BitStream::new(&mut somebits);
        match BITBUF_BITSIZE {
            #[cfg(target_pointer_width = "64")]
            64 => {
                assert_eq!(brdr.read_bits::<usize>(BITBUF_BITSIZE).unwrap(), 0x0102030405060708);
            }
            #[cfg(target_pointer_width = "32")]
            32 => {
                assert_eq!(brdr.read_bits::<usize>(BITBUF_BITSIZE).unwrap(), 0x01020304);
            }
            _ => unimplemented!()
        }
    }
}
