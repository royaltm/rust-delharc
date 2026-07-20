use core::fmt;
#[cfg(feature = "std")]
use std::io;

/// An error variant type returned from
/// [`LhaHeader::parse_os_type()`](super::LhaHeader::parse_os_type()).
///
/// Contains the original OS ID byte.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UnrecognizedOsType(pub u8);

/// Enumeration of all known operating system identifiers, which indicate 
/// the origin O/S of an archive file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
#[repr(u8)]
#[derive(Default)]
pub enum OsType {
    #[default]
    /// OS is unkown, usually found in LHA level 0 headers
    Generic =            0x00,
    /// Microsoft MS/DOS
    MsDos =              b'M',
    /// Microsoft Windows 95
    Win95 =              b'w',
    /// Microsoft Windows NT
    WinNt =              b'W',
    /// Unix
    Unix =               b'U',
    /// IBM OS/2
    Os2 =                b'2',
    /// Macintosh OS developped by Apple
    MacOs =              b'm',
    /// Amiga OS
    Amiga =              b'A',
    /// Atari TOS
    Atari =              b'a',
    /// Java Virtual Machine
    Java =               b'J',
    /// Digital Research CP/M
    Cpm =                b'C',
    /// Digital Research FlexOS
    FlexOs =             b'F',
    /// Fujitsu Runser for FM-7
    Runser =             b'R',
    /// Fujitsu FM Towns OS
    TownsOs =            b'T',
    /// Microware OS-9
    Os9 =                b'9',
    /// Microware OS-9/68k
    Osk =                b'K',
    /// OS/386 (?)
    Os386 =              b'3',
    /// Sharp X68000 Human68K OS
    Human68k =           b'H',
    /// OS-9/X68000
    Xosk =               b'X',
    /// An identifier produced by LHARK tool which runs under DOS
    Lhark =              b' '
}

impl From<OsType> for u8 {
    fn from(ostype: OsType) -> u8 {
        ostype as u8
    }
}

impl TryFrom<u8> for OsType {
    type Error = UnrecognizedOsType;
    fn try_from(ostype: u8) -> Result<Self, Self::Error> {
        Ok(match ostype {
            0x00 => OsType::Generic,
            b'M' => OsType::MsDos,
            b'w' => OsType::Win95,
            b'W' => OsType::WinNt,
            b'U' => OsType::Unix,
            b'2' => OsType::Os2,
            b'm' => OsType::MacOs,
            b'A' => OsType::Amiga,
            b'a' => OsType::Atari,
            b'J' => OsType::Java,
            b'C' => OsType::Cpm,
            b'F' => OsType::FlexOs,
            b'R' => OsType::Runser,
            b'T' => OsType::TownsOs,
            b'9' => OsType::Os9,
            b'K' => OsType::Osk,
            b'3' => OsType::Os386,
            b'H' => OsType::Human68k,
            b'X' => OsType::Xosk,
            b' ' => OsType::Lhark,
            _ => return Err(UnrecognizedOsType(ostype))
        })
    }
}

impl From<OsType> for &'static str {
    fn from(ostype: OsType) -> &'static str {
        match ostype {
            OsType::Generic => "-",
            OsType::MsDos => "MS-DOS",
            OsType::Win95 => "Win9x",
            OsType::WinNt => "WinNT",
            OsType::Unix => "UNIX",
            OsType::Os2 => "OS/2",
            OsType::MacOs => "MacOS",
            OsType::Amiga => "Amiga",
            OsType::Atari => "Atari",
            OsType::Java => "Java",
            OsType::Cpm => "CP/M",
            OsType::FlexOs => "FLEX",
            OsType::Runser => "Runser",
            OsType::TownsOs => "TownsOS",
            OsType::Os9 => "OS-9",
            OsType::Osk => "OS-9/68K",
            OsType::Os386 => "OS-386",
            OsType::Human68k => "Human68K",
            OsType::Xosk => "OS-9/X68",
            OsType::Lhark => "LHARK",
        }
    }
}

impl fmt::Display for OsType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        <&str>::from(*self).fmt(f)
    }
}

impl core::error::Error for UnrecognizedOsType {}

impl fmt::Display for UnrecognizedOsType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Unrecognized OS type: {}", self.0)
    }
}

#[cfg(feature = "std")]
impl From<UnrecognizedOsType> for io::Error {
    fn from(e: UnrecognizedOsType) -> Self {
        io::Error::new(io::ErrorKind::InvalidData, e)
    }
}

#[cfg(test)]
mod tests {
    #[cfg(not(feature = "std"))]
    use alloc::string::ToString;
    use super::*;

    #[test]
    fn ostype_works() {
        let oses = [
            0x00,
            b'M',
            b'w',
            b'W',
            b'U',
            b'2',
            b'm',
            b'A',
            b'a',
            b'J',
            b'C',
            b'F',
            b'R',
            b'T',
            b'9',
            b'K',
            b'3',
            b'H',
            b'X',
            b' ',
        ];
        for osbyte in oses {
            let os = OsType::try_from(osbyte).unwrap();
            assert_eq!(u8::from(os), osbyte);
        }
        let err = OsType::try_from(0xff).unwrap_err();
        assert!(err.to_string().starts_with("Unrecognized OS type: "));
        #[cfg(feature = "std")]
        assert!(std::io::Error::from(err).to_string().starts_with("Unrecognized OS type: "));
    }
}
