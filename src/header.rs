//! # **LHA** header and related types.
#[cfg(feature = "std")]
use std::path::PathBuf;
#[cfg(feature = "std")]
use std::borrow::Cow;
#[cfg(not(feature = "std"))]
use alloc::{boxed::Box, string::String, borrow::Cow};
use chrono::{LocalResult, prelude::*};

mod compression;
mod msdos;
mod os_9;
mod ostype;
mod parser;
mod timestamp;
mod unix;

use parser::ext::*;

pub use compression::*;
pub use msdos::*;
pub use os_9::*;
pub use ostype::*;
pub use parser::*;
pub use timestamp::*;
pub use unix::*;

/// An object representing a partially parsed LHA header.
///
/// This object can be obtained from the [`LhaHeader::read`] function.
#[derive(Debug, Clone, PartialEq, Eq)]
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
    ///
    /// Extended area is only present on header levels 0 and 1.
    ///
    /// Some tools store the first byte of this area to OS ID in level 0 headers.
    pub extended_area: Box<[u8]>,
    /// The size of the first extra header.
    ///
    /// The extra headers are only present on header levels 1 and above.
    pub first_header_len: u32,
    /// The extra headers' data.
    ///
    /// The extra headers are only present on header levels 1 and above.
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
    /// Return whether the archive is an empty directory or a symbolic link.
    pub fn is_directory(&self) -> bool {
        self.compression_method().ok()
            .map(CompressionMethod::is_directory)
            .unwrap_or(false)
    }
    /// Attempt to parse the `os_type` field and return the `OsType` enum on success.
    ///
    /// Header level 0 doesn't have an OS ID field, but in this instance a first byte of
    /// the extended area, if present, will be parsed as an `os_type` field.
    /// Only UNIX and OS/9 will be recognized in this way.
    pub fn parse_os_type(&self) -> Result<OsType, UnrecognizedOsType> {
        if self.level > 0 {
            OsType::try_from(self.os_type)
        }
        else if let Some(&byte) = self.extended_area.first()
            // PMarc stores comment in the extended area
            && !self.compression.starts_with(b"-pm")
        {
            OsType::try_from(byte).map(|os| if matches!(os, OsType::Unix|OsType::Os9|OsType::Osk) {
                os
            }
            else {
                OsType::Generic
            })
        }
        else {
            Ok(OsType::Generic)
        }
    }
    /// Attempt to parse the extra headers, extended area and as a last resort
    /// the `last_modified` field taking into account the header level, and on
    /// success return an instance of [`DateTime<Utc>`][DateTime] or a
    /// [`NaiveDateTime`] wrapped in an `TimestampResult` enum.
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
            // parse leve 0 timestamp from extended area for UNIX and OS-9/68k
            if self.level == 0 &&
               let Ok(OsType::Unix)|Ok(OsType::Osk) = self.parse_os_type() &&
               self.extended_area.len() >= 12 &&
               self.extended_area[1] == 0
            {
                let ts = read_u32(&self.extended_area[2..6]).unwrap();
                return Utc.timestamp_opt(ts as i64, 0).into()
            }
            parse_msdos_datetime(self.last_modified).into()
        }
        else {
            Utc.timestamp_opt(self.last_modified as i64, 0).into()
        }
    }
    /// Attempt to parse the `compression` method field and return the `CompressionMethod` enum on success.
    pub fn compression_method(&self) -> Result<CompressionMethod, UnrecognizedCompressionMethod> {
        CompressionMethod::try_from(&self.compression)
    }
    /// Attempt to parse the `filename` field and search extended headers for
    /// the directory and an alternative file name field and return a complete
    /// path to the file or a directory.
    ///
    /// This function converts all non-ASCII or control characters to `%xx`
    /// sequences and all system specific directory separator characters to `_`
    /// in file names.
    ///
    /// Malicious path components, like `..`, `.` or `//` are stripped from the
    /// path names.
    ///
    /// If the `filename` field is empty, but there exists a non-empty directory
    /// header, the returned path will end with the directory separator. This
    /// may indicate that the entry was intended as a directory, but other
    /// fields, like the size and compression field should also be consulted.
    ///
    /// # Notes
    /// * If the path name could not be found the returned `PathBuf` will be empty.
    /// * Some filesystems may still reject the file or path names if path names
    ///   include some forbidden characters, e.g. `?` or `*` in `Windows`.
    /// * This method makes its best effort to return a non-absolute path name,
    ///   however it is not guaranteed, so make sure the path is not absolute
    ///   before creating a file or a directory.
    /// * If the archive OS is [`OsType::Amiga`] the file name parsing terminates
    ///   before the `nul` character.
    ///
    /// It is known that some early Amiga archivers created a directory entry
    /// with `-lh0` and not `-lhd-` compression type. In this instance the
    /// returned path from this function  will end with a directory separator.
    ///
    /// To check if the [`Path`] ends with a directory separator, call
    /// [`Path::has_trailing_sep()`] or you can polyfill the function until
    /// it's going to be stabilized with:
    /// ```ignore
    /// path.as_os_str().as_encoded_bytes().last()
    ///     .is_some_and(|&s| s == std::path::MAIN_SEPARATOR_STR.as_bytes()[0]));
    /// ```
    ///
    /// # `no_std`
    ///
    /// This method is only available with `std` feature enabled.
    #[cfg(feature = "std")]
    #[cfg_attr(docsrs, doc(cfg(feature = "std")))]
    pub fn parse_pathname(&self) -> PathBuf {
        let mut path = PathBuf::new();
        let mut filename = Cow::Borrowed("");
        for header in self.iter_extra() {
            match header {
                [EXT_HEADER_FILENAME, data @ ..] => {
                    let nilterm = self.parse_os_type() == Ok(OsType::Amiga);
                    filename = parse_str_nilterm(data, nilterm, false);
                },
                [EXT_HEADER_PATH, data @ ..] => {
                    parse_pathname(data, &mut path);
                }
                _ => {}
            }
        }
        if filename.is_empty() {
            let data = if self.filename.is_empty() {
                if !path.as_os_str().is_empty() {
                    path.push(""); /* end the path with a separator */
                }
                return path
            }
            else if self.parse_os_type() == Ok(OsType::Amiga) {
                split_data_at_nil_or_end(&self.filename).0
            }
            else {
                &self.filename
            };
            parse_pathname(data, &mut path);
            // level 0|1 filename field ends with / ?
            if !path.as_os_str().is_empty() &&
               let Some(&last) = data.last() && is_separator(last.into())
            {
                path.push(""); /* end the path with a separator */
            }
        }
        else {
            path.push(filename.as_ref());
        }
        path
    }
    /// Attempt to parse the `filename` field and search extended headers for
    /// the directory and an alternative file name field and return a complete
    /// path to the file or a directory, separated by '`/`' characters.
    /// 
    /// This method is like [`LhaHeader::parse_pathname`] but will return a
    /// `String` instead of a `PathBuf` and can be used without the `std`
    /// feature enabled.
    ///
    /// If the `filename` field is empty, but there exists a non-empty directory
    /// header, the returned path will end with the '`/`' character. This
    /// may indicate that the entry was intended as a directory, but other
    /// fields, like the size and compression field should also be consulted.
    pub fn parse_pathname_to_str(&self) -> String {
        let mut path = String::new();
        let mut filename = Cow::Borrowed("");
        for header in self.iter_extra() {
            match header {
                [EXT_HEADER_FILENAME, data @ ..] => {
                    let nilterm = self.parse_os_type() == Ok(OsType::Amiga);
                    filename = parse_str_nilterm(data, nilterm, false);
                },
                [EXT_HEADER_PATH, data @ ..] => {
                    parse_pathname_to_str(data, &mut path);
                }
                _ => {}
            }
        }
        if filename.is_empty() {
            let data = if self.filename.is_empty() {
                if !path.is_empty() {
                    path.push('/'); /* end the path with a separator */
                }
                return path
            }
            else if self.parse_os_type() == Ok(OsType::Amiga) {
                split_data_at_nil_or_end(&self.filename).0
            }
            else {
                &self.filename
            };
            parse_pathname_to_str(data, &mut path);
            // level 0|1 filename field ends with / ?
            if !path.is_empty() &&
               let Some(&last) = data.last() && is_separator(last.into())
            {
                path.push('/'); /* end the path with a separator */
            }
        }
        else {
            if !path.is_empty() {
                path.push('/');
            }
            path.push_str(filename.as_ref());
        }
        path
    }
    /// Attempts to find and return the file comment field in extended header data.
    ///
    /// The routine converts all non-ASCII or control characters to `%xx` sequences.
    ///
    /// # Notes
    /// Some archives made on [`OsType::Amiga`] can have a comment embedded in the filename field
    /// after the `nul` character. If the comment could not be found in extended data, an attempt
    /// is made to extract the comment from the filename if the archive OS supports it.
    ///
    /// If the compression type is `-pm?-` returns the whole extended area as a comment.
    pub fn parse_comment(&self) -> Option<Cow<'_, str>> {
        let mut raw_filename = &self.filename[..];
        for header in self.iter_extra() {
            match header {
                [EXT_HEADER_FILENAME, data @ ..] => {
                    raw_filename = data;
                },
                [EXT_HEADER_COMMENT, data @ ..] => {
                    let comment = parse_str_nilterm(data, false, true);
                    if !comment.is_empty() {
                        return Some(comment)
                    }
                }
                _ => {}
            }
        }
        if self.parse_os_type() == Ok(OsType::Amiga) {
            split_data_at_nil_or_end(raw_filename)
            .1
            .map(|data| parse_str_nilterm(data, false, true))
        }
        else if self.compression.starts_with(b"-pm") && !self.extended_area.is_empty() {
            let comment = parse_str_nilterm(&*self.extended_area, false, true);
            (!comment.is_empty()).then_some(comment)
        }
        else {
            None
        }
    }
    /// Attempt to parse the extra headers, the extended area of header levels 0,
    /// to find the unix User-ID and Group-ID fields, and on success return a
    /// tuple of `(UID, GID)`.
    ///
    /// # Note
    /// The UID and GID values should be considered with a reservation, especially
    //  if an archive was not created on a UNIX operating system.
    pub fn parse_unix_uid_gid(&self) -> Option<(u16, u16)> {
        for header in self.iter_extra() {
            if let [EXT_HEADER_UNIX_UIDGID, data @ ..] = header &&
               data.len() >= 4
            {
                let (gid, uid) = data[0..4].split_at(2);
                let gid = read_u16(&gid).unwrap();
                let uid = read_u16(&uid).unwrap();
                return Some((uid, gid))
            }
        }
        if self.level == 0 &&
           self.extended_area.len() >= 12 &&
           let Ok(OsType::Unix|OsType::Osk) = self.parse_os_type()
        {
            let len = self.extended_area.len();
            let (uid, gid) = self.extended_area[len - 4..len].split_at(2);
            let uid = read_u16(&uid).unwrap();
            let gid = read_u16(&gid).unwrap();
            return Some((uid, gid))
        }
        None
    }
    /// Attempt to parse the extra headers, the extended area of header level 0,
    /// to find the unix permissions, and on success return an instance of
    /// [`Permissions`] flags.
    ///
    /// # Note
    /// The [`Permissions`] object properly identifies permission flags only
    /// if the file was created on UNIX, OS-9 or OS-9/68K operating systems.
    ///
    /// The permissions are converted from OS-9 attributes on non UNIX
    /// originating file.
    pub fn parse_unix_permissions(&self) -> Option<Permissions> {
        match self.parse_extended_attrs()? {
            Ok(perm) => Some(perm),
            Err(os9_attr) => Some(os9_attr.into())
        }
    }
    /// Attempt to parse the extra headers, the extended area of header level 0,
    /// to find the unix permissions, and on success return an instance of
    /// [`Os9Attrs`] flags.
    ///
    /// # Note
    /// The [`Os9Attrs`] object properly identifies attributes flags only
    /// if the file was created on OS-9 or OS-9/68K operating systems.
    pub fn parse_os_9_attrs(&self) -> Option<Os9Attrs> {
        match self.parse_extended_attrs()? {
            Err(os9_attr) => Some(os9_attr),
            Ok(..) => None
        }
    }

    fn parse_extended_attrs(&self) -> Option<Result<Permissions, Os9Attrs>> {
        let mut perm_raw = None;
        let mut is_os9 = false;
        for header in self.iter_extra() {
            match header {
                [EXT_HEADER_UNIX_PERM, data @ ..] if data.len() >= 2 => {
                    perm_raw = Some(read_u16(&data[0..2]).unwrap());
                    is_os9 = matches!(self.parse_os_type(), Ok(OsType::Osk|OsType::Os9));
                    break
                }
                [EXT_HEADER_OS9, data @ ..] if data.len() >= 12 => {
                    perm_raw = Some(data[7].into());
                    is_os9 = true;
                    break
                }
                _ => {}
            }
        }

        if perm_raw.is_none() &&
           self.level == 0 &&
           let Ok(os_type) = self.parse_os_type()
        {
            let data = &*self.extended_area;
            let len = data.len();
            // smell the header type
            match os_type {
                OsType::Unix|OsType::Osk if matches!(len, 12|16) && data[1] == 0 => {
                    let offs = len - 6;
                    perm_raw = read_u16(&data[offs..offs + 2]);
                    is_os9 = os_type == OsType::Osk;
                }
                OsType::Os9 if len >= 22 &&
                               data[9] == EXT_HEADER_OS9 &&
                               data[1] == data[17] &&
                               data[2] == data[18] =>
                {
                    perm_raw = Some(data[1].into());
                    is_os9 = true;
                }
                _ => {}
            }
        }

        if is_os9 {
            perm_raw.map(|p| p as u8) // truncate unused bits
                    .map(Os9Attrs::from_bits_truncate)
                    .map(Err)
        }
        else {
            perm_raw.map(Permissions::from_bits_truncate)
                    .map(Ok)
        }
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
