//! PMarc decoders
//!
//! Original C version: (c) 2011, 2012, Simon Howard lhasa/lib/pma_common.c
//!
//! Rust version: (c) 2026, Rafał Michalski
use crate::{
    bitstream::BitRead,
    error::LhaError,
    stub_io::Read,
};

mod pm1;
mod pm2;
mod history_list;

#[cfg_attr(docsrs, doc(cfg(feature = "pm")))]
pub use pm1::*;
#[cfg_attr(docsrs, doc(cfg(feature = "pm")))]
pub use pm2::*;
use history_list::*;
use super::unsafe_assert;

/// This object is used to decode variable bit integer encoding
#[derive(Debug, Clone, Copy)]
struct VarLenEntry {
    /// The base value to which the read value needs to be added
    offs: u16,
    /// The number of bits to read the value from
    bits: u16,
}

/// A read wrapper for PMarc version 1 which when the end of file
/// is reached yields zeroes forever instead of ending the stream.
///
/// There seem to be archive files that actually depend on this
/// ability to read "beyond" the length of the compressed data.
#[derive(Debug)]
#[repr(transparent)]
struct NoEofReader<R>(R);

impl VarLenEntry {
    const fn new(offs: u16, bits: u16) -> Self {
        VarLenEntry { offs, bits }
    }
    // Read a variable length code from the bit stream
    #[inline]
    fn decode_variable_length<R: BitRead>(&self, br: &mut R) -> Result<u16, LhaError<R::Error>> {
        let value: u16 = br.read_bits(self.bits.into())?;
        Ok(value + self.offs)
    }
}

impl<R: Read> Read for NoEofReader<R> {
    type Error = R::Error;

    #[inline(always)]
    fn unexpected_eof() -> Self::Error {
        R::unexpected_eof()
    }
    /// Read until EOF and fill the rest of the buffer with 0
    fn read_all(&mut self, buf: &mut[u8]) -> Result<usize, Self::Error> {
        let n = self.0.read_all(buf)?;
        if n < buf.len() {
            buf[n..].fill(0);
        }
        Ok(buf.len())

    }
    #[inline]
    fn read_exact(&mut self, buf: &mut [u8]) -> Result<(), Self::Error> {
        self.read_all(buf)?;
        Ok(())
    }
}
