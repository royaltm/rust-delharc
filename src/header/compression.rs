use core::fmt;
use core::convert::TryFrom;
#[cfg(feature = "std")]
use std::error::Error;
#[cfg(feature = "std")]
use std::io;

#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompressionMethod {
    /// Special "meta" method marking a directory. Also used for symlinks.
    Lhd,
    Lzs,
    Lz4,
    Lz5,
    Lh0,
    Lh1,
    Lh4,
    Lh5,
    Lh6,
    Lh7,
    Lhx,
    Pm0,
    Pm1,
    Pm2,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UnrecognizedCompressionMethod([u8;5]);

impl TryFrom<&[u8;5]> for CompressionMethod {
    type Error = UnrecognizedCompressionMethod;
    fn try_from(s: &[u8;5]) -> Result<Self, Self::Error> {
        Ok(match s {
            b"-lhd-" => CompressionMethod::Lhd,
            b"-lzs-" => CompressionMethod::Lzs,
            b"-lz4-" => CompressionMethod::Lz4,
            b"-lz5-" => CompressionMethod::Lz5,
            b"-lh0-" => CompressionMethod::Lh0,
            b"-lh1-" => CompressionMethod::Lh1,
            b"-lh4-" => CompressionMethod::Lh4,
            b"-lh5-" => CompressionMethod::Lh5,
            b"-lh6-" => CompressionMethod::Lh6,
            b"-lh7-" => CompressionMethod::Lh7,
            b"-lhx-" => CompressionMethod::Lhx,
            b"-pm0-" => CompressionMethod::Pm0,
            b"-pm1-" => CompressionMethod::Pm1,
            b"-pm2-" => CompressionMethod::Pm2,
            _ => return Err(UnrecognizedCompressionMethod(*s))
        })
    }
}

impl CompressionMethod {
    pub fn is_directory(&self) -> bool {
        if let CompressionMethod::Lhd = self {
            return true
        }
        false
    }

    pub fn as_identifier(self) -> &'static [u8;5] {
        match self {
            CompressionMethod::Lhd => b"-lhd-",
            CompressionMethod::Lzs => b"-lzs-",
            CompressionMethod::Lz4 => b"-lz4-",
            CompressionMethod::Lz5 => b"-lz5-",
            CompressionMethod::Lh0 => b"-lh0-",
            CompressionMethod::Lh1 => b"-lh1-",
            CompressionMethod::Lh4 => b"-lh4-",
            CompressionMethod::Lh5 => b"-lh5-",
            CompressionMethod::Lh6 => b"-lh6-",
            CompressionMethod::Lh7 => b"-lh7-",
            CompressionMethod::Lhx => b"-lhx-",
            CompressionMethod::Pm0 => b"-pm0-",
            CompressionMethod::Pm1 => b"-pm1-",
            CompressionMethod::Pm2 => b"-pm2-",
        }
    }
}

#[cfg(feature = "std")]
impl Error for UnrecognizedCompressionMethod {}

impl fmt::Display for UnrecognizedCompressionMethod {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Unrecognized compression method: {:x?}", self.0)
    }
}

impl fmt::Display for CompressionMethod {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let strid = self.as_identifier();
        assert!(strid.is_ascii());
        unsafe {
            core::str::from_utf8_unchecked(strid)
        }.fmt(f)
    }
}

#[cfg(feature = "std")]
impl From<UnrecognizedCompressionMethod> for io::Error {
    fn from(e: UnrecognizedCompressionMethod) -> Self {
        io::Error::new(io::ErrorKind::InvalidData, e)
    }
}
