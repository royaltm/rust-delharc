use core::{fmt, error};
#[cfg(feature = "std")]
use std::{io, collections::TryReserveError};
#[cfg(not(feature = "std"))]
use alloc::collections::TryReserveError;
use crate::stub_io::Read;

/// The result returned from functions in this crate.
///
/// `R` is the type of the data reader object.
pub type LhaResult<T, R> = Result<T, LhaError<<R as Read>::Error>>;

/// `delharc` error enum.
///
/// With `std` feature enabled `E` is [`std::io::Error`] and
/// [`LhaError`] can be converted to [`std::io::Error`] using
/// [`From`] or [`Into`].
#[derive(Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum LhaError<E> {
    /// An error occured while reading from the data stream
    Io(E),
    /// An error occured when parsing LHA header
    HeaderParse(LhaHeaderError),
    /// An error occured when decompressing a file
    Decompress(DecompressionError),
    /// A checksum mismatch error occured
    Checksum,
}

/// An enum of [`LhaHeader`] errors
#[derive(Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum LhaHeaderError {
    /// The header level is unknown
    UnknownLevel,
    /// First 2 bytes of level 3 header are incorrect
    Level3Signature,
    /// The extended header is too small
    ExtendedHeaderSize,
    /// A wrapping sum (level 0 and 1) did not match calculated value
    WrappingSumMismatch,
    /// A common header CRC-16 did not match calculated value
    Crc16Mismatch,
    /// Header size validation failed (level 0 and 1)
    SizeMismatch,
    /// Header long size validation failed (level 2 and 3)
    LongSizeMismatch,
    /// Header skip size validation failed (level 1)
    SkipSizeMismatch,
    /// Another common CRC-16 header found
    CommonHeader,
    /// A header was expected but 0 was encountered or end of stream
    HeaderNotFound,
    /// Memory allocation for extra header data has failed
    OutOfMemory,
}

/// An enum of errors returned from the static Huffman Tree building method.
#[derive(Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum BuildError {
    /// The tree code length slice is longer than the maximum
    /// number of potential leaf values.
    CodeLengthOverflow,
    /// There are not enough leaf nodes to cover the last tree level
    LeavesUndeflow,
    /// There are too many leaf nodes provided in code lengths
    LeavesOverflow,
    /// Allocating memory for tree nodes has failed
    OutOfMemory,
}

/// An enum of errors returned from decompression algorithms.
#[derive(Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum DecompressionError {
    /// Attempted to decompress a file with unsupported compression method
    UnsupportedCompression,
    /// LHv2/PMarc-v2 - too many code lengths requested for a command tree
    CommandCodeTableOverflow,
    /// LHv2 - too many code lengths requested for a history offset tree
    OffsetCodeTableOverflow,
    /// LHv2/PMarc-v2 - a requested single command code is too large
    CommandOverflow,
    /// LHv2 - a requested single history offset code is too large
    OffsetOverflow,
    /// LHv2 - too large code length decoded from the bit stream
    CodeLengthOverflow,
    #[cfg(feature = "pm")]
    #[cfg_attr(docsrs, doc(cfg(feature = "pm")))]
    /// PMarc - too large history distance
    HistoryDistanceOverflow,
    /// Bit-stream - too many bits requested for a given integer type capacity
    BitSizeOverflow,
    /// An error occured while building a Huffman Tree
    Tree(BuildError),
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

impl fmt::Display for BuildError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use BuildError::*;
        match self {
            CodeLengthOverflow => "too many code lengths",
            LeavesUndeflow => "not enough leaf nodes in code lengths",
            LeavesOverflow => "too many leaf nodes in code lengths",
            OutOfMemory => "memory allocation failed",
        }
        .fmt(f)
    }
}

impl error::Error for BuildError {}

impl From<TryReserveError> for BuildError {
    fn from(_err: TryReserveError) -> BuildError {
        BuildError::OutOfMemory
    }
}

impl fmt::Display for DecompressionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use DecompressionError::*;
        match self {
            UnsupportedCompression => "unsupported compression method",
            CommandCodeTableOverflow => "commands code length table is too large",
            OffsetCodeTableOverflow => "offset code length table is too large",
            CommandOverflow => "command code is too large",
            OffsetOverflow => "offset code is too large",
            #[cfg(feature = "pm")]
            HistoryDistanceOverflow => "history distance is too large",
            CodeLengthOverflow => "code length is too large",
            BitSizeOverflow => "too many bits requested",
            Tree(err) => return write!(f, "while building a tree: {}", err),
        }
        .fmt(f)
    }
}

impl error::Error for DecompressionError {}

impl From<BuildError> for DecompressionError {
    fn from(err: BuildError) -> DecompressionError {
        DecompressionError::Tree(err)
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
            HeaderParse(e) => Some(e),
            Decompress(e) => Some(e),
            _ => None
        }
    }
}

impl<E> From<LhaHeaderError> for LhaError<E> {
    fn from(err: LhaHeaderError) -> LhaError<E> {
        LhaError::HeaderParse(err)
    }
}

impl<E> From<DecompressionError> for LhaError<E> {
    fn from(err: DecompressionError) -> LhaError<E> {
        LhaError::Decompress(err)
    }
}

impl<E> From<BuildError> for LhaError<E> {
    fn from(err: BuildError) -> LhaError<E> {
        LhaError::Decompress(err.into())
    }
}

#[cfg(feature = "std")]
impl From<io::Error> for LhaError<io::Error> {
    fn from(err: io::Error) -> LhaError<io::Error> {
        LhaError::Io(err)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(feature = "std")]
    #[test]
    fn errors_works() {
        use core::error::Error;
        let mut vec = <Vec<u8>>::new();
        assert_eq!(LhaHeaderError::try_from(vec.try_reserve(usize::MAX).unwrap_err()).unwrap(),
                   LhaHeaderError::OutOfMemory);
        assert_eq!(BuildError::try_from(vec.try_reserve(usize::MAX).unwrap_err()).unwrap(),
                   BuildError::OutOfMemory);
        assert_eq!(LhaHeaderError::OutOfMemory.to_string(), "memory allocation failed");
        assert_eq!(BuildError::OutOfMemory.to_string(), "memory allocation failed");

        assert_eq!(LhaError::from(std::io::Error::other("I/O error")).to_string(), "I/O error");
        assert_eq!(LhaError::<std::io::Error>::from(LhaHeaderError::OutOfMemory).to_string(),
                   "while parsing LHA header: memory allocation failed");
        assert_eq!(DecompressionError::from(BuildError::OutOfMemory).to_string(),
                   "while building a tree: memory allocation failed");
        assert_eq!(LhaError::<std::io::Error>::from(DecompressionError::UnsupportedCompression).to_string(),
                   "while decompressing: unsupported compression method");
        assert_eq!(LhaError::<std::io::Error>::from(BuildError::OutOfMemory).to_string(),
                   "while decompressing: while building a tree: memory allocation failed");
        assert_eq!(LhaError::<std::io::Error>::Checksum.to_string(),
                   "CRC-16 file checksum mismatch");

        let err = LhaError::<std::io::Error>::Checksum;
        assert!(err.source().is_none());
        let err = LhaError::Io(std::io::Error::other("error"));
        assert!(err.source().unwrap().downcast_ref::<std::io::Error>().is_some());
        let err: LhaError<std::io::Error> = LhaError::HeaderParse(LhaHeaderError::OutOfMemory);
        assert!(err.source().unwrap().downcast_ref::<LhaHeaderError>().is_some());
        let err: LhaError<std::io::Error> = LhaError::Decompress(DecompressionError::UnsupportedCompression);
        assert!(err.source().unwrap().downcast_ref::<DecompressionError>().is_some());

        assert_eq!(LhaError::from(std::io::Error::other("error")).to_string(), "error");
    }

    #[test]
    fn error_messages_works() {
        #[cfg(not(feature = "std"))]
        use alloc::string::ToString;

        assert_eq!(LhaError::<&str>::from(LhaHeaderError::UnknownLevel).to_string(),
                   "while parsing LHA header: unknown header level");
        assert_eq!(LhaError::<&str>::from(LhaHeaderError::Level3Signature).to_string(),
                   "while parsing LHA header: level 3 signature mismatch");
        assert_eq!(LhaError::<&str>::from(LhaHeaderError::ExtendedHeaderSize).to_string(),
                   "while parsing LHA header: not enough bytes in the extended header");
        assert_eq!(LhaError::<&str>::from(LhaHeaderError::WrappingSumMismatch).to_string(),
                   "while parsing LHA header: wrapping checksum mismatch");
        assert_eq!(LhaError::<&str>::from(LhaHeaderError::Crc16Mismatch).to_string(),
                   "while parsing LHA header: CRC-16 checksum mismatch");
        assert_eq!(LhaError::<&str>::from(LhaHeaderError::SizeMismatch).to_string(),
                   "while parsing LHA header: size validation failed");
        assert_eq!(LhaError::<&str>::from(LhaHeaderError::LongSizeMismatch).to_string(),
                   "while parsing LHA header: long size validation failed");
        assert_eq!(LhaError::<&str>::from(LhaHeaderError::SkipSizeMismatch).to_string(),
                   "while parsing LHA header: skip size validation failed");
        assert_eq!(LhaError::<&str>::from(LhaHeaderError::CommonHeader).to_string(),
                   "while parsing LHA header: duplicate CRC-16 header found");
        assert_eq!(LhaError::<&str>::from(LhaHeaderError::HeaderNotFound).to_string(),
                   "while parsing LHA header: header not found");
        assert_eq!(LhaError::<&str>::from(LhaHeaderError::OutOfMemory).to_string(),
                   "while parsing LHA header: memory allocation failed");

        assert_eq!(LhaError::<&str>::from(BuildError::CodeLengthOverflow).to_string(),
                   "while decompressing: while building a tree: too many code lengths");
        assert_eq!(LhaError::<&str>::from(BuildError::LeavesUndeflow).to_string(),
                   "while decompressing: while building a tree: not enough leaf nodes in code lengths");
        assert_eq!(LhaError::<&str>::from(BuildError::LeavesOverflow).to_string(),
                   "while decompressing: while building a tree: too many leaf nodes in code lengths");
        assert_eq!(LhaError::<&str>::from(BuildError::OutOfMemory).to_string(),
                   "while decompressing: while building a tree: memory allocation failed");

        assert_eq!(LhaError::<&str>::from(DecompressionError::UnsupportedCompression).to_string(),
                   "while decompressing: unsupported compression method");
        assert_eq!(LhaError::<&str>::from(DecompressionError::CommandCodeTableOverflow).to_string(),
                   "while decompressing: commands code length table is too large");
        assert_eq!(LhaError::<&str>::from(DecompressionError::OffsetCodeTableOverflow).to_string(),
                   "while decompressing: offset code length table is too large");
        assert_eq!(LhaError::<&str>::from(DecompressionError::CommandOverflow).to_string(),
                   "while decompressing: command code is too large");
        assert_eq!(LhaError::<&str>::from(DecompressionError::OffsetOverflow).to_string(),
                   "while decompressing: offset code is too large");
        #[cfg(feature = "pm")]
        assert_eq!(LhaError::<&str>::from(DecompressionError::HistoryDistanceOverflow).to_string(),
                   "while decompressing: history distance is too large");
        assert_eq!(LhaError::<&str>::from(DecompressionError::CodeLengthOverflow).to_string(),
                   "while decompressing: code length is too large");
        assert_eq!(LhaError::<&str>::from(DecompressionError::BitSizeOverflow).to_string(),
                   "while decompressing: too many bits requested");
        assert_eq!(LhaError::<&str>::Io("other error").to_string(), "other error");
    }
}
