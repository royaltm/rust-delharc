use core::fmt;
#[cfg(feature = "std")]
use std::io;

/// Enumeration of all known compression methods
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompressionMethod {
    /// Special "meta" method marking a directory. Also used for symlinks.
    Lhd,
    /// A variant for `-lzs-` compression method
    Lzs,
    /// A variant for `-lz4-` compression method (no compression)
    Lz4,
    /// A variant for `-lz5-` compression method
    Lz5,
    /// A variant for `-lh0-` compression method (no compression)
    Lh0,
    /// A variant for `-lh1-` compression method
    Lh1,
    /// A variant for `-lh4-` compression method
    Lh4,
    /// A variant for `-lh5-` compression method
    Lh5,
    /// A variant for `-lh6-` compression method
    Lh6,
    /// A variant for `-lh7-` compression method
    Lh7,
    /// A variant for `-lhx-` compression method
    Lhx,
    /// A variant for `-pm0-` compression method (no compression)
    Pm0,
    /// A variant for `-pm1-` compression method
    Pm1,
    /// A variant for `-pm2-` compression method
    Pm2,
}

/// An error variant type returned from
/// [`LhaHeader::compression_method()`](super::LhaHeader::compression_method()).
///
/// Contains the original compression identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UnrecognizedCompressionMethod(pub [u8;5]);

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
    /// Return whether the compression method indicates a directory entry
    #[inline]
    pub fn is_directory(self) -> bool {
        matches!(self, CompressionMethod::Lhd)
    }

    /// Return whether the compression method actually compresses data
    #[inline]
    pub fn is_compressed(self) -> bool {
        !matches!(self, CompressionMethod::Lhd|
                        CompressionMethod::Lz4|
                        CompressionMethod::Lh0|
                        CompressionMethod::Pm0)
    }

    /// Return the signature of the compression method
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

impl core::error::Error for UnrecognizedCompressionMethod {}

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
            // SAFETY: compression identifiers are ASCII-only,
            // validated with assert above
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

#[cfg(test)]
mod tests {
    #[cfg(not(feature = "std"))]
    use alloc::string::ToString;
    use super::*;

    #[test]
    fn header_compression_works() {
        let methods = [
            (b"-lhd-", true,  false),
            (b"-lzs-", false, true),
            (b"-lz4-", false, false),
            (b"-lz5-", false, true),
            (b"-lh0-", false, false),
            (b"-lh1-", false, true),
            (b"-lh4-", false, true),
            (b"-lh5-", false, true),
            (b"-lh6-", false, true),
            (b"-lh7-", false, true),
            (b"-lhx-", false, true),
            (b"-pm0-", false, false),
            (b"-pm1-", false, true),
            (b"-pm2-", false, true),
        ];
        for (m, is_dir, is_comp) in methods {
            let cm = CompressionMethod::try_from(m).unwrap();
            assert_eq!(cm.as_identifier(), m);
            assert_eq!(cm.is_directory(), is_dir);
            assert_eq!(cm.is_compressed(), is_comp);
            assert_eq!(cm.to_string().as_str(), <str>::from_utf8(m).unwrap());
        }
        let err = CompressionMethod::try_from(b"-xxx-").unwrap_err();
        assert!(err.to_string().starts_with("Unrecognized compression method: "));
        #[cfg(feature = "std")]
        assert!(std::io::Error::from(err).to_string().starts_with("Unrecognized compression method: "));
    }
}
