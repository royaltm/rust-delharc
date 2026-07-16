use core::{fmt, error};
#[cfg(feature = "std")]
use std::{io, collections::TryReserveError};
#[cfg(not(feature = "std"))]
use alloc::collections::TryReserveError;
use crate::stub_io::Read;

pub type LhaResult<T, R> = Result<T, LhaError<<R as Read>::Error>>;

/// `delharc` error object.
///
/// With `std` feature enabled `E` is [`std::io::Error`] and
/// `LhaError` can be converted to [`std::io::Error`] using [`From`] or [`Into`].
#[derive(Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum LhaError<E> {
    /// I/O error.
    Io(E),
    /// When parsing LHA header.
    HeaderParse(LhaHeaderError),
    /// When decompressing a file.
    Decompress(DecompressionError),
    /// File checksum mismatch.
    Checksum,
}

/// An enum of [`LhaHeader`] errors.
#[derive(Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum LhaHeaderError {
    /// Unknown header level
    UnknownLevel,
    /// Level 3 signature mismatch
    Level3Signature,
    /// Not enough bytes in the extended header
    ExtendedHeaderSize,
    /// Wrapping checksum mismatch
    WrappingSumMismatch,
    /// CRC-16 checksum mismatch
    Crc16Mismatch,
    /// Size validation failed
    SizeMismatch,
    /// Long size validation failed
    LongSizeMismatch,
    /// Skip size validation failed
    SkipSizeMismatch,
    /// Duplicate common CRC-16 header found
    CommonHeader,
    /// Header not found
    HeaderNotFound,
    /// Memory allocation failed
    OutOfMemory,
}

/// An enum of errors returned from decompression algorithms.
#[derive(Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum DecompressionError {
    // "unsupported compression method"
    UnsupportedCompression,
    // "too many tree code lengths"
    CodeLengthTableOverflow,
    // "not enough leaf nodes in code lengths"
    TreeLeavesUndeflow,
    // "too many leaf nodes in code lengths"
    TreeLeavesOverflow,
    // "temporary code length table size overflow"
    TemporaryCodeTableOverflow,
    // "commands code length table size overflow"
    CommandCodeTableOverflow,
    // "offset code length table size overflow"
    OffsetCodeTableOverflow,
    // "command code overflow"
    CommandOverflow,
    // "offset code overflow"
    OffsetOverflow,
    // "code length overflow"
    CodeLengthOverflow,
    // "too many bits requested"
    BitSizeOverflow,
    /// Memory allocation failed
    OutOfMemory,
}

impl fmt::Display for LhaHeaderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use LhaHeaderError::*;
        match self {
            UnknownLevel => "unknown header level",
            Level3Signature => "level 3 signature mismatch",
            ExtendedHeaderSize => "not enough bytes in the extended header",
            WrappingSumMismatch => "wrapping checksum mismatch",
            Crc16Mismatch => "CRC-16 checksum mismatch",
            SizeMismatch => "size validation failed",
            LongSizeMismatch => "long size validation failed",
            SkipSizeMismatch => "skip size validation failed",
            CommonHeader => "duplicate CRC-16 header found",
            HeaderNotFound => "header not found",
            OutOfMemory => "memory allocation failed",
        }
        .fmt(f)
    }
}

impl error::Error for LhaHeaderError {}

impl From<TryReserveError> for LhaHeaderError {
    fn from(_err: TryReserveError) -> LhaHeaderError {
        LhaHeaderError::OutOfMemory
    }
}

impl fmt::Display for DecompressionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use DecompressionError::*;
        match self {
            UnsupportedCompression => "unsupported compression method",
            CodeLengthTableOverflow => "too many tree code lengths",
            TreeLeavesUndeflow => "not enough leaf nodes in code lengths",
            TreeLeavesOverflow => "too many leaf nodes in code lengths",
            TemporaryCodeTableOverflow => "temporary code length table size overflow",
            CommandCodeTableOverflow => "commands code length table size overflow",
            OffsetCodeTableOverflow => "offset code length table size overflow",
            CommandOverflow => "command code overflow",
            OffsetOverflow => "offset code overflow",
            CodeLengthOverflow => "code length overflow",
            BitSizeOverflow => "too many bits requested",
            OutOfMemory => "memory allocation failed",
        }
        .fmt(f)
    }
}

impl error::Error for DecompressionError {}

impl From<TryReserveError> for DecompressionError {
    fn from(_err: TryReserveError) -> DecompressionError {
        DecompressionError::OutOfMemory
    }
}

impl<E: fmt::Display> fmt::Display for LhaError<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use LhaError::*;
        match self {
            Io(e) => e.fmt(f),
            HeaderParse(e) => write!(f, "while parsing LHA header: {}", e),
            Decompress(e) => write!(f, "while decompressing: {}", e),
            Checksum => write!(f, "CRC-16 file checksum mismatch"),
        }
    }
}

impl<E: error::Error + 'static> error::Error for LhaError<E> {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
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
            err => Error::new(ErrorKind::InvalidData, err),
        }
    }
}
