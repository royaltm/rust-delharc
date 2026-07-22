//! # Bit-stream tools.
use crate::error::{LhaResult, LhaError, DecompressionError};
use crate::stub_io::Read;

type BitBuf = usize;
const BITBUF_BYTESIZE: usize = size_of::<BitBuf>();
const BITBUF_BITSIZE: u32 = BitBuf::BITS;

/// This trait is implemented for all primitives that can receive bits using
/// [`BitRead::read_bits()`].
pub trait UBits: Copy {
    /// The size of this integer type in bits
    const BITS: u32;
    /// Convert the bit buffer to this integer value by truncating unused bits
    fn from_bits(bitbuf: BitBuf) -> Self;
}

/// This interface is for reading individual bits from a data stream.
pub trait BitRead {
    /// The error type returned from the underlying data reader.
    type Error;
    /// Read the next single bit from the stream. Return `true` if the bit
    /// is `1` and `false` if it's `0`.
    fn read_bit(&mut self) -> Result<bool, LhaError<Self::Error>>;
    /// Reads the next `n` bits from the stream.
    ///
    /// For example reading 4 bits into the `u8` type will result in: `0b0000abcd` where
    /// `a`, `b`, `c`, `d` are consecutive MSB -> LSB bits that were read from the source.
    ///
    /// Returns `0` if `n` is `0` without reading from the underlying reader.
    ///
    /// # Errors
    /// Returns an error if `n` exceed the bit capacity of `T`.
    fn read_bits<T: UBits>(&mut self, n: u32) -> Result<T, LhaError<Self::Error>>;
    /// Creates a "by reference" adaptor for this instance of `BitRead`.
    /// The returned adaptor also implements `BitRead` and will simply borrow this current reader.
    #[allow(dead_code)]
    fn by_ref(&mut self) -> &mut Self {
        self
    }
}

/// A simple bit-stream reader, wrapped over a readable stream.
///
/// Bits are being read from consecutive data bytes, starting
/// from the highest bit of each byte.
#[derive(Debug)]
pub struct BitStream<R> {
    inner: R,
    // x..x10..0
    bits_buf: BitBuf,
}

macro_rules! impl_ubits {
    ($ty:ty) => {
        impl UBits for $ty {
            const BITS: u32 = <$ty>::BITS;
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
#[cfg(feature = "extend")]
impl_ubits!(u64);
#[cfg(feature = "extend")]
impl_ubits!(u128);

impl<R: Read> BitStream<R> {
    /// Create and return a new `BitStream`.
    pub fn new(inner: R) -> BitStream<R> {
        BitStream { inner, bits_buf: 1 << (BITBUF_BITSIZE - 1) }
    }
    /// Unwrap this `BitStream`, returning the underlying reader.
    ///
    /// Note that any leftover data in the internal bit buffer is lost.
    /// Therefore, a following read from the underlying reader may lead to
    /// data loss.
    pub fn into_inner(self) -> R {
        self.inner
    }
    /// Get a reference to the underlying reader.
    ///
    /// Care should be taken to avoid modifying the internal I/O state of the
    /// underlying readers as doing so may corrupt the internal state of this
    /// `BitStream`.
    pub fn get_ref(&self) -> &R {
        &self.inner
    }
    /// Get a mutable reference to the underlying reader.
    ///
    /// Care should be taken to avoid modifying the internal I/O state of the
    /// underlying readers as doing so may corrupt the internal state of this
    /// `BitStream`.
    pub fn get_mut(&mut self) -> &mut R {
        &mut self.inner
    }
    /// The callers must take care to provide `n` in the range `1..=BITBUF_BITSIZE`.
    #[inline]
    fn next_bits(&mut self, n: u32) -> LhaResult<BitBuf, R> {
        debug_assert!(n != 0 && n <= BITBUF_BITSIZE);
        let have_bits = BITBUF_BITSIZE - self.bits_buf.trailing_zeros() - 1;
        let res = self.bits_buf >> (BITBUF_BITSIZE - n);

        if n <= have_bits {
            self.bits_buf <<= n;
            return Ok(res)
        }

        let missing_bits = n - have_bits;
        let mut buf = [0u8;BITBUF_BYTESIZE];
        let bits_read = 8u32 * self.inner.read_all(&mut buf)
                                .map_err(LhaError::Io)? as u32;
        if bits_read < missing_bits {
            return Err(LhaError::Io(R::unexpected_eof()))
        }
        let new_bits: BitBuf = BitBuf::from_be_bytes(buf);
        // clear trailing bits and merge
        let res = res & (res - 1)
                | new_bits >> (BITBUF_BITSIZE - missing_bits);
        self.bits_buf = if missing_bits == BITBUF_BITSIZE {
            0
        }
        else {
            new_bits << missing_bits
        } | 1 << (BITBUF_BITSIZE - 1 - (bits_read - missing_bits));
        Ok(res)
    }

    // #[inline]
    // fn read_exact_or_to_end(&mut self, mut buf: &mut[u8]) -> io::Result<usize> {
    //     let orig_len = buf.len();
    //     while !buf.is_empty() {
    //         match self.inner.read(buf) {
    //             Ok(0) => break,
    //             Ok(n) => buf = &mut buf[n..],
    //             Err(ref e) if e.kind() == io::ErrorKind::Interrupted => {}
    //             Err(e) => return Err(e),
    //         }
    //     }
    //     Ok(orig_len - buf.len())
    // }
}

impl<R: BitRead> BitRead for &mut R {
    type Error = R::Error;

    #[inline]
    fn read_bit(&mut self) -> Result<bool, LhaError<Self::Error>> {
        (*self).read_bit()
    }

    #[inline]
    fn read_bits<T: UBits>(&mut self, n: u32) -> Result<T, LhaError<Self::Error>> {
        (*self).read_bits(n)
    }
}

impl<R: Read> BitRead for BitStream<R> {
    type Error = R::Error;

    #[inline]
    fn read_bit(&mut self) -> Result<bool, LhaError<Self::Error>> {
        self.next_bits(1).map(|bits| bits != 0)
    }

    fn read_bits<T: UBits>(&mut self, n: u32) -> Result<T, LhaError<Self::Error>> {
        match n {
            0 => Ok(0),
            n if n <= T::BITS => self.next_bits(n),
            _ => Err(LhaError::Decompress(DecompressionError::BitSizeOverflow))
        }.map(T::from_bits)
    }
}

#[cfg(feature = "std")]
#[cfg(test)]
mod tests {
    use std::io;
    use super::*;
    #[test]
    fn bit_stream_works() {
        assert_eq!(BITBUF_BYTESIZE, size_of::<usize>());
        assert!(BITBUF_BITSIZE >= 32);
        assert_eq!(BITBUF_BITSIZE, BITBUF_BYTESIZE as u32 * 8);
        let mut somebits: &[u8] = &[];
        let mut brdr = BitStream::new(&mut somebits);
        let err: io::Error = brdr.read_bit().unwrap_err().into();
        assert_eq!(err.kind(), io::ErrorKind::UnexpectedEof);
        let err: io::Error = brdr.read_bits::<usize>(BITBUF_BITSIZE).unwrap_err().into();
        assert_eq!(err.kind(), io::ErrorKind::UnexpectedEof);
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
        let err: io::Error = brdr.read_bit().unwrap_err().into();
        assert_eq!(err.kind(), io::ErrorKind::UnexpectedEof);

        let mut somebits: &[u8] = &[1,2,3,4,5,6,7,8];
        let mut brdr = BitStream::new(&mut somebits);
        assert_eq!(brdr.get_ref(), &&mut &[1,2,3,4,5,6,7,8]);
        assert_eq!(brdr.get_mut(), &mut &mut &[1,2,3,4,5,6,7,8]);
        assert!(matches!(brdr.by_ref(), &mut BitStream {..}));
        assert!(matches!((&mut brdr.by_ref()).read_bits::<usize>(u32::MAX).unwrap_err(),
                    LhaError::Decompress(DecompressionError::BitSizeOverflow)));
        assert!(matches!((&mut brdr.by_ref()).read_bits::<u8>(u8::BITS + 1).unwrap_err(),
                    LhaError::Decompress(DecompressionError::BitSizeOverflow)));
        assert!(matches!((&mut brdr.by_ref()).read_bits::<u16>(u16::BITS + 1).unwrap_err(),
                    LhaError::Decompress(DecompressionError::BitSizeOverflow)));
        assert!(matches!((&mut brdr.by_ref()).read_bits::<u32>(u32::BITS + 1).unwrap_err(),
                    LhaError::Decompress(DecompressionError::BitSizeOverflow)));

        #[cfg(target_pointer_width = "64")]
        {
            assert_eq!(BITBUF_BITSIZE, 64);
            assert_eq!(brdr.read_bits::<usize>(BITBUF_BITSIZE).unwrap(), 0x0102030405060708);
        }
        #[cfg(target_pointer_width = "32")]
        {
            assert_eq!(BITBUF_BITSIZE, 32);
            assert_eq!(brdr.read_bits::<usize>(BITBUF_BITSIZE).unwrap(), 0x01020304);
        }
    }
}
