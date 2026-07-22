//! Proxy `I/O` tools.
use core::cmp;
#[cfg(not(feature = "std"))]
use core::fmt;
#[cfg(feature = "std")]
use std::io;

/// This trait is an implementation vessel that bridges the `std` and `no_std` version
/// of this library.
///
/// With `std` feature enabled this trait is implemented for all types that implement
/// [`io::Read`].
///
/// Without `std` this trait is only implemented for `&[u8]` and can be implemented for
/// other types as well.
///
/// When using with `std` this trait should be ignored by user code and instead the callers
/// should rely on the `std` [`io::Read`] trait.
///
/// When using without `std` this trait should be imported and used with [`crate::LhaDecodeReader`]
/// as an interface for reading decompressed files.
pub trait Read {
    /// The error type returned by implementations.
    type Error;

    /// This method shall produce the "Unexpected EOF" error.
    fn unexpected_eof() -> Self::Error;
    /// Read data from the stream, filling the `buf` and return the number of bytes read.
    ///
    /// Similar to [`io::Read::read`] but continue on [`io::ErrorKind::Interrupted`].
    ///
    /// If the returned value is lower than the length of the `buf`, that indicates
    /// the EOF has been reached.
    fn read_all(&mut self, buf: &mut[u8]) -> Result<usize, Self::Error>;
    /// Exactly like [`io::Read::read_exact`].
    fn read_exact(&mut self, buf: &mut [u8]) -> Result<(), Self::Error> {
        if buf.len() != self.read_all(buf)? {
            Err(Self::unexpected_eof())
        }
        else {
            Ok(())
        }
    }
    /// Similar to [`io::Read::take`] but return a replacement [`Take`] struct.
    fn take(self, limit: u64) -> Take<Self>
        where Self: Sized
    {
        Take { inner: self, limit }
    }
    /// Creates a "by reference" adaptor for this instance of `Read`.
    #[inline]
    fn by_ref(&mut self) -> &mut Self {
       self
    }
}

pub(crate) fn discard_to_end<R: Read, const BUF: usize>(rd: &mut R) -> Result<(), R::Error> {
    use core::mem::MaybeUninit;
    assert!(BUF != 0);
    // FIXME: use BorrowedBuf once it stablizes
    let mut data = MaybeUninit::<[u8; BUF]>::uninit();
    let buf: &mut [u8; BUF] = unsafe {
        // SAFETY: data has not been initialized but is never read,
        // assuming Read implementation never reads from the buf
        data.assume_init_mut()
    };
    while 0 != rd.read_all(buf)? {}
    Ok(())
}

/// A replacement of [`io::Take`] that is used internally by `delharc`.
#[derive(Debug)]
pub struct Take<R> {
    limit: u64,
    inner: R,
}

impl<R> Take<R> {
    #[inline]
    /// Returns the number of bytes that can be read before this instance will
    /// return EOF.
    ///
    /// # Note
    /// This instance may reach EOF after reading fewer bytes than indicated by
    /// this method if the underlying [`Read`] instance reaches EOF.
    pub fn limit(&self) -> u64 {
        self.limit
    }
    /// Consumes the `Take`, returning the wrapped reader.
    pub fn into_inner(self) -> R {
        self.inner
    }
    /// Get a reference to the underlying reader.
    ///
    /// Care should be taken to avoid modifying the internal I/O state of the
    /// underlying reader as doing so may corrupt the internal limit of this
    /// `Take`.
    pub fn get_ref(&self) -> &R {
        &self.inner
    }
    /// Get a mutable reference to the underlying reader.
    ///
    /// Care should be taken to avoid modifying the internal I/O state of the
    /// underlying reader as doing so may corrupt the internal limit of this
    /// `Take`.
    pub fn get_mut(&mut self) -> &mut R {
        &mut self.inner
    }
}

impl<R: Read> Read for Take<R> {
    type Error = R::Error;

    #[inline]
    fn unexpected_eof() -> Self::Error {
        R::unexpected_eof()
    }

    #[inline]
    fn read_all(&mut self, buf: &mut [u8]) -> Result<usize, Self::Error> {
        // Don't call into inner reader at all at EOF because it may still block
        if self.limit == 0 {
            return Ok(0);
        }

        let max = cmp::min(buf.len() as u64, self.limit) as usize;
        let n = self.inner.read_all(&mut buf[..max])?;
        self.limit = self.limit.checked_sub(n as u64).expect("number of read bytes exceeds limit");
        Ok(n)
    }

    #[inline]
    fn read_exact(&mut self, buf: &mut [u8]) -> Result<(), Self::Error> {
        let len = buf.len();
        if len as u64 > self.limit {
            return Err(Self::unexpected_eof());
        }
        self.inner.read_exact(buf)?;
        self.limit -= len as u64;
        Ok(())        
    }
}

#[cfg(feature = "std")]
impl<R: io::Read> Read for R {
    type Error = io::Error;

    fn unexpected_eof() -> Self::Error {
        io::Error::new(io::ErrorKind::UnexpectedEof, "failed to fill whole buffer")
    }

    fn read_all(&mut self, mut buf: &mut[u8]) -> Result<usize, Self::Error> {
        let orig_len = buf.len();
        loop {
            match self.read(buf) {
                Ok(0) => break,
                Ok(n) if n < buf.len() => {
                    buf = &mut buf[n..];
                },
                Ok(..) => return Ok(orig_len),
                Err(ref e) if e.kind() == io::ErrorKind::Interrupted => {}
                Err(e) => return Err(e)
            }
        }
        Ok(orig_len - buf.len())
    }

    #[inline]
    fn read_exact(&mut self, buf: &mut [u8]) -> Result<(), Self::Error> {
        io::Read::read_exact(self, buf)
    }
}

/// An error when reading from slice without `std`.
#[cfg(not(feature = "std"))]
#[cfg_attr(docsrs, doc(cfg(not(feature = "std"))))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UnexpectedEofError;

#[cfg(not(feature = "std"))]
impl core::error::Error for UnexpectedEofError {}

#[cfg(not(feature = "std"))]
impl<R: Read + ?Sized> Read for &mut R {
    type Error = R::Error;

    fn unexpected_eof() -> Self::Error {
        R::unexpected_eof()
    }

    fn read_all(&mut self, buf: &mut[u8]) -> Result<usize, Self::Error> {
        R::read_all(*self, buf)
    }

    fn read_exact(&mut self, buf: &mut[u8]) -> Result<(), Self::Error> {
        R::read_exact(*self, buf)
    }
}

#[cfg(not(feature = "std"))]
impl<R: Read + ?Sized> Read for alloc::boxed::Box<R> {
    type Error = R::Error;

    fn unexpected_eof() -> Self::Error {
        R::unexpected_eof()
    }

    fn read_all(&mut self, buf: &mut[u8]) -> Result<usize, Self::Error> {
        R::read_all(self, buf)
    }

    fn read_exact(&mut self, buf: &mut[u8]) -> Result<(), Self::Error> {
        R::read_exact(self, buf)
    }
}

#[cfg(not(feature = "std"))]
impl fmt::Display for UnexpectedEofError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("failed to fill whole buffer")
    }
}

#[cfg(not(feature = "std"))]
impl Read for &'_[u8] {
    type Error = UnexpectedEofError;

    #[inline]
    fn unexpected_eof() -> Self::Error {
        UnexpectedEofError
    }

    #[inline]
    fn read_all(&mut self, buf: &mut[u8]) -> Result<usize, Self::Error> {
        let amt = cmp::min(buf.len(), self.len());
        let (a, b) = self.split_at(amt);

        // First check if the amount of bytes we want to read is small:
        // `copy_from_slice` will generally expand to a call to `memcpy`, and
        // for a single byte the overhead is significant.
        if amt == 1 {
            buf[0] = a[0];
        } else {
            buf[..amt].copy_from_slice(a);
        }

        *self = b;
        Ok(amt)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(not(feature = "std"))]
    #[test]
    fn stub_io_no_std_works() {
        use alloc::{boxed::Box, string::ToString};
        assert_eq!((&[][..]).read_all(&mut[]).unwrap(), 0);
        assert_eq!((&[][..]).read_all(&mut[0]).unwrap(), 0);
        assert_eq!((&[][..]).read_exact(&mut[0]).unwrap_err(), UnexpectedEofError);
        assert_eq!((&[][..]).take(0).read_exact(&mut[0]).unwrap_err(), UnexpectedEofError);
        let mut data: Box<&[u8]> = Box::new(&[]);
        assert_eq!(data.read_all(&mut[]).unwrap(), 0);
        assert_eq!(data.read_all(&mut[0]).unwrap(), 0);
        assert_eq!(data.read_exact(&mut[0]).unwrap_err(), UnexpectedEofError);
        assert_eq!(<Box<&[u8]> as Read>::unexpected_eof(), UnexpectedEofError);
        let data: &mut &mut _ = &mut &mut data;
        assert_eq!(data.read_all(&mut[]).unwrap(), 0);
        assert_eq!(data.read_all(&mut[0]).unwrap(), 0);
        assert_eq!(data.read_exact(&mut[0]).unwrap_err(), UnexpectedEofError);
        assert_eq!(<&mut &[u8] as Read>::unexpected_eof(), UnexpectedEofError);
        assert_eq!(UnexpectedEofError.to_string(), "failed to fill whole buffer");
    }

    #[cfg(feature = "std")]
    #[test]
    fn stub_io_std_works() {
        use std::io;
        #[derive(Debug, Eq, PartialEq)]
        struct Interrupted(bool);
        impl io::Read for Interrupted {
            fn read(&mut self, _buf: &mut[u8]) -> io::Result<usize> {
                if !self.0 {
                    self.0 = true;
                    Err(io::ErrorKind::Interrupted.into())
                }
                else {
                    Err(io::ErrorKind::UnexpectedEof.into())
                }
            }
        }
        assert_eq!((&[][..]).read_all(&mut[]).unwrap(), 0);
        assert_eq!((&[][..]).read_all(&mut[0]).unwrap(), 0);
        assert_eq!(Interrupted(false).read_all(&mut[0]).unwrap_err().kind(), io::ErrorKind::UnexpectedEof);
        assert_eq!(Interrupted(true).take(0).get_ref(), &Interrupted(true));
        assert_eq!(Interrupted(true).take(0).get_mut(), &mut Interrupted(true));
    }
}