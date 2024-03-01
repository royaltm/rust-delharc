use core::fmt;
#[cfg(feature = "std")]
use std::error::Error;
#[cfg(feature = "std")]
use std::io;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UnrecognizedOsType(pub u8);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
#[repr(u8)]
#[derive(Default)]
pub enum OsType {
    #[default]
    Generic =            0x00,
    MsDos =              b'M',
    Win95 =              b'w',
    WinNt =              b'W',
    Unix =               b'U',
    Os2 =                b'2',
    MacOs =              b'm',
    Amiga =              b'A',
    Atari =              b'a',
    Java =               b'J',
    Cpm =                b'C',
    FlexOs =             b'F',
    Runser =             b'R',
    TownsOs =            b'T',
    Os9 =                b'9',
    Osk =                b'K',
    Os386 =              b'3',
    Human68k =           b'H',
    Xosk =               b'X',
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
            _ => return Err(UnrecognizedOsType(ostype))
        })
    }
}

#[cfg(feature = "std")]
impl Error for UnrecognizedOsType {}

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
