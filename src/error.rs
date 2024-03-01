use core::fmt;
#[cfg(feature = "std")]
use std::io;
use crate::stub_io::Read;

pub type LhaResult<T, R> = Result<T, LhaError<<R as Read>::Error>>;

#[derive(Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum LhaError<E> {
    /// I/O error
    Io(E),
    /// Error parsing LHA header
    HeaderParse(&'static str),
    /// Error decompressing file
    Decompress(&'static str),
    /// Checksum error
    Checksum(&'static str),
}

impl<E: fmt::Display> fmt::Display for LhaError<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use LhaError::*;
        match self {
            Io(e) => e.fmt(f),
            HeaderParse(e) => write!(f, "while parsing header: {}", e),
            Decompress(e) => write!(f, "while decompressing: {}", e),
            Checksum(e) => write!(f, "checksum mismatch: {}", e),
        }
    }
}

#[cfg(feature = "std")]
impl<E: std::error::Error + 'static> std::error::Error for LhaError<E> {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        use LhaError::*;
        match self {
            Io(e) => Some(e),
            _ => None
        }
    }
}

#[cfg(feature = "std")]
impl From<LhaError<io::Error>> for io::Error {
    fn from(err: LhaError<io::Error>) -> Self {
        use LhaError::*;
        use io::{Error, ErrorKind};
        match err {
            Io(e) => e,
            HeaderParse(e) => Error::new(ErrorKind::InvalidData, e),
            Decompress(e) => Error::new(ErrorKind::InvalidData, e),
            Checksum(e) => Error::new(ErrorKind::InvalidData, e),
        }
    }
}
