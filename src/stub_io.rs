//! Proxy `I/O` tools.
use core::cmp;
#[cfg(not(feature = "std"))]
use core::fmt;
#[cfg(feature = "std")]
use std::io;

/// This trait is an implementation vessel that bridges the `std` and `no-std` version
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
    /// Similar to [`io::Read::read`] but continue on [`io::ErrorKind::Interrupted`].
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
    /// Similar to [`io::Read::take`] but return a replacement `Take` struct.
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
    use core::mem::{self, MaybeUninit};
    assert!(BUF != 0);
    // Create an uninitialized array of `MaybeUninit`. The `assume_init` is
    // safe because the type we are claiming to have initialized here is a
    // bunch of `MaybeUninit`s, which do not require initialization.
    let mut data: [MaybeUninit<u8>; BUF] = unsafe {
        MaybeUninit::uninit().assume_init()
    };
    let buf = {
        // TODO: use BorrowedBuf once it stablizes
        // We never read data and u8 doesn't implement Drop.
        unsafe { mem::transmute::<_, &mut[u8]>(&mut data[..]) }
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
    pub fn limit(&self) -> u64 {
        self.limit
    }

    pub fn into_inner(self) -> R {
        self.inner
    }

    pub fn get_ref(&self) -> &R {
        &self.inner
    }

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
        while !buf.is_empty() {
            match self.read(buf) {
                Ok(0) => break,
                Ok(n) => buf = &mut buf[n..],
                Err(ref e) if e.kind() == io::ErrorKind::Interrupted => {}
                Err(e) => return Err(e)
            }
        }
        Ok(orig_len - buf.len())
    }

    fn read_exact(&mut self, buf: &mut [u8]) -> Result<(), Self::Error> {
        io::Read::read_exact(self, buf)
    }
}

/// An error when reading from slice without `std`.
#[cfg(not(feature = "std"))]
#[derive(Debug)]
pub struct UnexpectedEofError;

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

    #[inline]
    fn read_exact(&mut self, buf: &mut [u8]) -> Result<(), Self::Error> {
        if buf.len() > self.len() {
            return Err(UnexpectedEofError);
        }
        self.read_all(buf)?;
        Ok(())
    }
}
