//! # **LHA** header and related types.
use core::convert::TryFrom;
use std::path::PathBuf;
use std::borrow::Cow;

use chrono::{LocalResult, prelude::*};

mod compression;
mod ostype;
mod msdos;
mod parser;
mod timestamp;

use parser::ext::*;

pub use msdos::*;
pub use compression::*;
pub use ostype::*;
pub use parser::*;
pub use timestamp::*;

/// Semi-parsed LHA header.
#[derive(Debug, Clone)]
pub struct LhaHeader {
    /// Header level: 0, 1, 2 or 3.
    pub level: u8,
    /// Raw compression identifier.
    pub compression: [u8;5],
    /// Compressed file size.
    pub compressed_size: u64,
    /// Original file size.
    pub original_size: u64,
    /// A raw filename for level 1 or 0 headers, might be empty. Always being empty for levels 2 or 3.
    ///
    /// In this instance the filename is stored in extra headers.
    pub filename: Box<[u8]>,
    /// MS-DOS attributes.
    pub msdos_attrs: MsDosAttrs,
    /// File's last modified date, format depends on the header level.
    ///
    /// * Level 0 and 1 - MS-DOS format (no time zone).
    /// * Level 2 and 3 - Unix timestamp (UTC).
    ///
    /// The "last modified" timestamp can also be found in the extended area and extra headers, as well as
    /// other kinds of timestamps (- last access, created).
    pub last_modified: u32,
    /// A raw OS-TYPE.
    pub os_type: u8,
    /// Uncompressed file's CRC-16.
    pub file_crc: u16,
    /// An extended area as raw bytes.
    pub extended_area: Box<[u8]>,
    /// The size of the first extra header.
    pub first_header_len: u32,
    /// The extra headers' data.
    pub extra_headers: Box<[u8]>,
}

impl Default for LhaHeader {
    fn default() -> Self {
        LhaHeader {
            level: 0,
            compression: [0;5],
            compressed_size: 0,
            original_size: 0,
            filename: Box::new([]),
            msdos_attrs: MsDosAttrs::ARCHIVE,
            last_modified: 0,
            os_type: 0,
            file_crc: 0,
            extended_area: Box::new([]),
            first_header_len: 0,
            extra_headers: Box::new([]),
        }
    }
}

impl LhaHeader {
    /// Returns `true` if the archive is an empty directory or a symbolic link.
    pub fn is_directory(&self) -> bool {
        self.compression_method().ok()
            .filter(CompressionMethod::is_directory)
            .is_some()
    }
    /// Attempts to parse the `os_type` field and returns the `OsType` enum on success.
    pub fn parse_os_type(&self) -> Result<OsType, UnrecognizedOsType> {
        OsType::try_from(self.os_type)
    }
    /// Attempts to parse the extended area, extra headers and as a last resort the `last_modified` field
    /// taking into account the header level, and on success returns an instance of [`DateTime<Utc>`][DateTime]
    /// or a [NaiveDateTime] wrapped in an `TimestampResult` enum.
    pub fn parse_last_modified(&self) -> TimestampResult {
        for header in self.iter_extra() {
            match header {
                [EXT_HEADER_UNIX_TIME, data @ ..] => {
                    if let Some(ts) = data.get(0..4).and_then(read_u32) {
                        return Utc.timestamp_opt(ts as i64, 0).into()
                    }
                }
                [EXT_HEADER_MSDOS_TIME, data @ ..] if data.len() == 24 => {
                    if let Some(mtime) = read_u64(&data[8..16]) {
                        return parse_win_filetime(mtime).into()
                    }
                }
                _ => {}
            }
        }
        if self.level < 2 {
            match self.parse_os_type() {
                Ok(OsType::Unix)|Ok(OsType::Osk) => {
                    if let Some(ts) = self.extended_area.get(1..5).and_then(read_u32) {
                        return Utc.timestamp_opt(ts as i64, 0).into()
                    }
                }
                _ => {}
            }
            parse_msdos_datetime(self.last_modified).into()
        }
        else {
            Utc.timestamp_opt(self.last_modified as i64, 0).into()
        }
    }
    /// Attempts to parse the `compression` method field and returns the `CompressionMethod` enum on success.
    pub fn compression_method(&self) -> Result<CompressionMethod, UnrecognizedCompressionMethod> {
        CompressionMethod::try_from(&self.compression)
    }
    /// Attempts to parse the `filename` field and searches the extended data for the directory and an
    /// alternative file name and returns a `PathBuf`.
    ///
    /// The routine converts all non-ascii or control ascii characters to `%xx` sequences and all system
    /// specific directory separator characters to `_` in file names.
    ///
    /// Malicious path components, like `..`, `.` or `//` are stripped from the path names.
    ///
    /// # Notes
    /// * If the path name could not be found the returned `PathBuf` will be empty.
    /// * Some filesystems may still reject the file or path names if path names include some forbidden
    ///   characters, e.g. `?` or `*` in `Windows`.
    /// * This method makes its best effort to return a non-absolute path name, however it is not guaranteed,
    ///   so make sure the path is not absolute before creating a file or a directory.
    pub fn parse_pathname(&self) -> PathBuf {
        let mut path = PathBuf::new();
        let mut filename = Cow::Borrowed("");
        for header in self.iter_extra() {
            match header {
                [EXT_HEADER_FILENAME, data @ ..] => {
                    filename = parse_filename(data)
                },
                [EXT_HEADER_PATH, data @ ..] => {
                    parse_pathname(data, &mut path);
                }
                _ => {}
            }
        }
        if filename.is_empty() {
            parse_pathname(&self.filename, &mut path);
        }
        else {
            path.push(filename.as_ref());
        }
        path
    }
}

/// Returns a `NaiveDateTime` on success from MS-DOS timestamp format.
///
/// ```text
/// bit   24       16        8        0
/// 76543210 76543210 76543210 76543210
/// YYYYYYYM MMMDDDDD hhhhhmmm mmmsssss
/// ```
///
/// | Sym. | Description                                 |
/// |------|---------------------------------------------|
/// | Y    | The year from 1980 (0 = 1980)               |
/// | M    | Month. [1, 12]                              |
/// | D    | Day. [1, 31]                                |
/// | h    | Hour. [0, 23].                              |
/// | m    | Minute. [0, 59].                            |
/// | s    | 2 seconds. [0, 29] (in units of 2 seconds). |
pub fn parse_msdos_datetime(ts: u32) -> Option<NaiveDateTime> {
    let sec = ts << 1 & 0x3e;
    let min = ts >> 5 & 0x3f;
    let hour = ts >> 11 & 0x1f;
    let day = ts >> 16 & 0x1f;
    let mon = ts >> 21 & 0xf;
    let year = 1980 + (ts >> 25 & 0x7f) as i32;
    NaiveDate::from_ymd_opt(year, mon, day).and_then(|d| d.and_hms_opt(hour, min, sec))
}

/// Returns a `DateTime<Utc>` on success from Windows [FILETIME] format.
///
/// [FILETIME]: https://docs.microsoft.com/en-us/windows/win32/api/minwinbase/ns-minwinbase-filetime
pub fn parse_win_filetime(filetime: u64) -> LocalResult<DateTime<Utc>> {
    if let Some(ft) = i64::try_from(filetime).ok().and_then(|ft|
                        ft.checked_sub(116_444_736_000_000_000))
    {
        let secs = ft / 10_000_000;
        let nanos = (ft % 10_000_000) as u32 * 100;
        return Utc.timestamp_opt(secs, nanos)
    }
    LocalResult::None
}
