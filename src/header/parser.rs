#[cfg(not(feature = "std"))]
use alloc::vec::Vec;
use core::{fmt::Write, num::Wrapping, slice};
use bytemuck::{NoUninit, AnyBitPattern, bytes_of_mut};
use crate::{
    error::{LhaError, LhaResult, LhaHeaderError},
    stub_io::Read,
    crc::Crc16,
};
use super::*;

/// Raw identifiers of extra headers.
pub mod ext {
    /// The "Common" header's CRC-16 field will always be reset to 0 in the parsed header data.
    /// This is the necessary condition to verify header's checksum.
    pub const EXT_HEADER_COMMON:       u8 = 0x00;
    /// The "File name" header may contain the entry's file name.
    pub const EXT_HEADER_FILENAME:     u8 = 0x01;
    /// The "Directory name" header may contain the directory of the entry.
    pub const EXT_HEADER_PATH:         u8 = 0x02;
    /// The "Multi-disc" header
    pub const EXT_HEADER_MULTI_DISC:   u8 = 0x39;
    /// The "Comment" header
    pub const EXT_HEADER_COMMENT:      u8 = 0x3F;
    /// The MS-DOS ["Attributes"](super::MsDosAttrs) header
    pub const EXT_HEADER_MSDOS_ATTRS:  u8 = 0x40;
    /// The "Windows time stamp" header
    pub const EXT_HEADER_WINDOWS_TIME: u8 = 0x41;
    /// An alias of [`EXT_HEADER_WINDOWS_TIME`]
    #[deprecated(note="please use `EXT_HEADER_WINDOWS_TIME` instead")]
    pub const EXT_HEADER_MSDOS_TIME:   u8 = EXT_HEADER_WINDOWS_TIME;
    /// The "File size" header with 64-bit file size information
    pub const EXT_HEADER_FILE_SIZES:   u8 = 0x42;
    /// An alias of [`EXT_HEADER_FILE_SIZES`]
    #[deprecated(note="please use `EXT_HEADER_FILE_SIZES` instead")]
    pub const EXT_HEADER_MSDOS_SIZE:   u8 = EXT_HEADER_FILE_SIZES;
    /// The UNIX ["Permission"](super::Permissions) header
    pub const EXT_HEADER_UNIX_PERM:    u8 = 0x50;
    /// The UNIX "GID UID" header
    pub const EXT_HEADER_UNIX_UIDGID:  u8 = 0x51;
    /// The UNIX "Group name" header
    pub const EXT_HEADER_UNIX_GROUP:   u8 = 0x52;
    /// The UNIX "User name" header
    pub const EXT_HEADER_UNIX_OWNER:   u8 = 0x53;
    /// The UNIX "Time stamp" header
    pub const EXT_HEADER_UNIX_TIME:    u8 = 0x54;
    /// The Mac "Capsule" header
    pub const EXT_HEADER_MAC_CAPSULE:  u8 = 0x7D;
    /// The OS/2 extended attributes header
    pub const EXT_HEADER_OS2_ATTR1:    u8 = 0x7E;
    /// Level 3 extended attributes header
    pub const EXT_HEADER_EXT_ATTRS:    u8 = 0x7F;
    /// The OS/9 extended attributes header
    pub const EXT_HEADER_OS9:          u8 = 0xCC;
    /// The metadata header, currently used by MorphOS to store file comments
    pub const EXT_HEADER_METADATA:     u8 = 0x71;
}

use ext::*;
/// An iterator through extra headers, yielding the headers' raw content excluding
/// the next header length field.
pub struct ExtraHeaderIter<'a> {
    data: &'a [u8],
    header_length: u32,
    header_len32: bool
}

impl<'a> Iterator for ExtraHeaderIter<'a> {
    type Item = &'a [u8];

    fn next(&mut self) -> Option<Self::Item> {
        let header_length = self.header_length as usize;
        if header_length == 0 {
            return None
        }
        let (res, data) = self.data.split_at(header_length);
        let (res, len) = if self.header_len32 {
            res.split_last_chunk::<4>().map(|(dat, &len)|
                (dat, u32::from_le_bytes(len)))
        }
        else {
            res.split_last_chunk::<2>().map(|(dat, &len)|
                (dat, u16::from_le_bytes(len).into()))
        }.unwrap();
        self.header_length = len;
        self.data = data;
        Some(res)
    }
}

/// Allocate at once this number of bytes maximum when reading variable size fields
const ALLOCATE_LIMIT_MAX: usize = 8*1024;

/// The raw LHA header fragment with a rigid structure
#[derive(Clone, Copy, Debug, Default, NoUninit, AnyBitPattern)]
#[repr(C)]
#[repr(packed)]
struct LhaRawBaseHeader {
    compression: [u8;5],
    compressed_size: [u8;4],
    original_size: [u8;4],
    last_modified: [u8;4],
    msdos_attrs: u8,
    lha_level: u8
}

/// The internal header parser object
struct Parser<'a, R> {
    rd: &'a mut R,
    /// A collected header's CRC-16 checksum
    crc: Crc16,
    /// A collected header's wrapping sum checksum
    csum: Wrapping<u8>,
    /// The number of bytes parsed so far
    len: usize
}

impl<R: Read> Parser<'_, R> {
    /// Read a next byte if there is one more in the stream increasing
    /// the parsed counter and updating the header CRC-16 checksum.
    ///
    /// NOTE: this function does not update the wrapping sum.
    fn read_u8_or_none(&mut self) -> LhaResult<Option<u8>, R> {
        let mut byte = 0u8;
        if 0 == self.rd.read_all(slice::from_mut(&mut byte)).map_err(LhaError::Io)? {
            return Ok(None)
        }
        self.update_checksums_no_wrapping_sum(slice::from_ref(&byte));
        Ok(Some(byte))
    }
    /// Read the next byte, increase the parsed counter and update all checksums
    fn read_u8(&mut self) -> LhaResult<u8, R> {
        let mut byte: u8 = 0;
        self.read_exact(slice::from_mut(&mut byte))?;
        Ok(byte)
    }
    /// Read the next 2 bytes, increase the parsed counter and update all checksums.
    /// Return an LE 16-bit value.
    fn read_u16(&mut self) -> LhaResult<u16, R> {
        let mut buf = [0u8;2];
        self.read_exact(&mut buf)?;
        Ok(u16::from_le_bytes(buf))
    }
    /// Read the next 4 bytes, increase the parsed counter and update all checksums.
    /// Return an LE 32-bit value.
    fn read_u32(&mut self) -> LhaResult<u32, R> {
        let mut buf = [0u8;4];
        self.read_exact(&mut buf)?;
        Ok(u32::from_le_bytes(buf))
    }
    /// Read the exact number of bytes, increase the parsed counter and update all
    /// checksums.
    fn read_exact(&mut self, buf: &mut [u8]) -> LhaResult<(), R> {
        self.rd.read_exact(buf).map_err(LhaError::Io)?;
        self.update_checksums(buf);
        Ok(())
    }
    /// Read the `limit` bytes into an newly allocated boxed slice, increase the
    /// parsed counter and update all checksums.
    fn read_limit(&mut self, limit: usize) -> LhaResult<Box<[u8]>, R> {
        let mut buf = Vec::new();
        self.read_limit_no_checksums(limit, &mut buf)?;
        self.update_checksums(&buf);
        Ok(buf.into_boxed_slice())
    }
    /// Increase the parser counter and update all header checksums from data
    fn update_checksums(&mut self, data: &[u8]) {
        self.update_checksums_no_wrapping_sum(data);
        self.csum = wrapping_csum(self.csum, data);
    }
    /// Increase the parser counter and update only the CRC-16 header checksum
    fn update_checksums_no_wrapping_sum(&mut self, data: &[u8]) {
        self.len += data.len();
        self.crc.digest(data);
    }
    /// Read the `limit` bytes into a vector.
    ///
    /// This function does not increase the parsed counter, nor updates any checksums.
    ///
    /// Take care not to allocate too much memory when doing so, until more data
    /// is read from the stream.
    fn read_limit_no_checksums(&mut self, mut limit: usize, buf: &mut Vec<u8>) -> LhaResult<(), R> {
        while limit != 0 {
            let chunk_size = limit.min(ALLOCATE_LIMIT_MAX);
            buf.try_reserve_exact(chunk_size).map_err(|err| LhaError::HeaderParse(err.into()))?;
            // FIXME: use BorrowedBuf once stabilized
            let spare_uninit = &mut buf.spare_capacity_mut()[..chunk_size];
            // SAFETY: assume read_exact is write-only
            let spare = unsafe { spare_uninit.assume_init_mut() };
            self.rd.read_exact(spare).map_err(LhaError::Io)?;
            // SAFETY: assume chunk_size was read into buf
            // this can't overflow because buf.len() + chunk_size <= buf.capacity()
            unsafe { buf.set_len(buf.len() + chunk_size); }
            limit -= chunk_size;
        }
        Ok(())
    }
}

impl LhaHeader {
    /// Attempt to parse the LHA header. Return `Ok(Some(LhaHeader))` on success. Return `Ok(None)`
    /// if the end of archive marker (a `0` byte) was encountered.
    ///
    /// The method validates all length and checksum fields of the header, but does not parse extra
    /// headers except:
    ///
    /// * The ["Common"][EXT_HEADER_COMMON] header for validating the header's CRC-16 checksum.
    /// * The ["MS-DOS Attributes"][EXT_HEADER_MSDOS_ATTRS] header for reading MS-DOS attributes.
    /// * The ["MS-DOS Size"][EXT_HEADER_FILE_SIZES] header for reading 64-bit file size.
    ///
    /// All extra header data is available as raw bytes and raw extra headers can be easily iterated
    /// with the [`LhaHeader::iter_extra`] function.
    ///
    /// [`LhaHeader`] methods can be further called on the returned object to attempt to parse the
    /// additional properties of an archive entry.
    ///
    /// # Errors
    /// Returns an error from the underlying reading operations or because a malformed header was
    /// encountered.
    pub fn read<R: Read>(rd: &mut R) -> LhaResult<Option<LhaHeader>, R> {
        let mut parser = Parser {
            rd, 
            crc: Crc16::default(),
            csum: Wrapping(0),
            len: 0
        };
        let header_len = match parser.read_u8_or_none()? {
            Some(0)|None => return Ok(None),
            Some(len) => len
        };
        let csum = parser.read_u8()?;
        // reset wrapping checksum which should not include the first 2 bytes
        parser.csum = Wrapping(0);

        let mut raw_header = LhaRawBaseHeader::default();
        parser.read_exact(bytes_of_mut(&mut raw_header))?;
        if raw_header.lha_level > 3 {
            return Err(LhaError::HeaderParse(LhaHeaderError::UnknownLevel))
        }

        // read filename if level 0 or 1
        let filename = if raw_header.lha_level < 2 {
            let filename_len = parser.read_u8()? as usize;
            if (header_len as usize) < parser.len + filename_len {
                return Err(LhaError::HeaderParse(LhaHeaderError::SizeMismatch))
            }
            parser.read_limit(filename_len)?
        }
        else {
            Box::new([])
        };

        // file CRC-16
        let file_crc = parser.read_u16()?;

        // OS-TYPE
        let mut os_type = 0;
        if raw_header.lha_level > 0 {
            os_type = parser.read_u8()?;
        }

        // extended area, only 0 and 1 level
        let mut extended_area: Box<[u8]> = Box::new([]);
        if raw_header.lha_level < 2 {
            let mut min_len = parser.len;
            if raw_header.lha_level == 0 {
                min_len -= 2; // no extra headers
            }
            let extended_len = (header_len as usize).checked_sub(min_len)
                              .ok_or(LhaHeaderError::SizeMismatch)?;
            if extended_len != 0 {
                extended_area = parser.read_limit(extended_len)?;
            }
        };

        // extra headers
        let mut long_header_len: u32 = 0; // a long header length found in level >= 2
        let mut first_header_len: u32 = 0;
        // establish the first extra header length and the long header length
        match raw_header.lha_level {
            1 => {
                first_header_len = parser.read_u16()? as u32;
            }
            2 => {
                long_header_len = u16::from_le_bytes([header_len, csum]) as u32;
                first_header_len = parser.read_u16()? as u32;
            }
            3 => {
                long_header_len = parser.read_u32()?;
                first_header_len = parser.read_u32()?;
                if header_len != 4 || csum != 0 {
                    return Err(LhaError::HeaderParse(LhaHeaderError::Level3Signature))
                }
            }
            _ => {}
        }

        // validate level 0 and 1 header checksum
        if raw_header.lha_level < 2 {
            if csum != parser.csum.0 {
                return Err(LhaError::HeaderParse(LhaHeaderError::WrappingSumMismatch))
            }
        }
        else if (long_header_len.saturating_sub(first_header_len) as usize) < parser.len {
            return Err(LhaError::HeaderParse(LhaHeaderError::LongSizeMismatch))
        }

        let mut extra_headers = Vec::new();
        let mut msdos_attrs = MsDosAttrs::from_bits_retain(raw_header.msdos_attrs as u16);
        let mut original_size = u32::from_le_bytes(raw_header.original_size) as u64;
        let mut compressed_size = u32::from_le_bytes(raw_header.compressed_size) as u64;
        let mut header_crc: Option<u16> = None;
        // read extra headers
        let min_header_len = if raw_header.lha_level == 3 { 5 } else { 3 };
        let mut extra_header_len = first_header_len as usize;
        while extra_header_len != 0 {
            if extra_header_len < min_header_len {
                return Err(LhaError::HeaderParse(LhaHeaderError::ExtendedHeaderSize))
            }
            // check long header length (level 2, 3)
            if long_header_len != 0 {
                if (long_header_len as usize).saturating_sub(extra_header_len - 2) < parser.len {
                    return Err(LhaError::HeaderParse(LhaHeaderError::LongSizeMismatch))
                }
            }
            else if compressed_size < (extra_headers.len() as u64) + extra_header_len as u64  {
                // otherwise check skip size (level 1)
                return Err(LhaError::HeaderParse(LhaHeaderError::SkipSizeMismatch))
            }
            parser.read_limit_no_checksums(extra_header_len, &mut extra_headers)?;
            let start = extra_headers.len() - extra_header_len;
            let header = &mut extra_headers[start..];
            match header {
                // we need to extract the CRC-16 from header and clear it in order to calculate checksum
                [EXT_HEADER_COMMON, data @ ..] => {
                    if header_crc.is_some() {
                        return Err(LhaError::HeaderParse(LhaHeaderError::CommonHeader))
                    }
                    if let Some(crc) = data.get_mut(0..2) {
                        header_crc = read_u16(crc);
                        for p in crc.iter_mut() {
                            *p = 0;
                        }
                    }
                }
                [EXT_HEADER_MSDOS_ATTRS, data @ ..]|
                [EXT_HEADER_EXT_ATTRS,   data @ ..] if data.len() >= 2 => {
                    if let Some(attrs) = read_u16(&data[0..2]) {
                        msdos_attrs = MsDosAttrs::from_bits_retain(attrs);
                    }
                }
                [EXT_HEADER_FILE_SIZES, data @ ..] if raw_header.lha_level >= 2 && data.len() >= 16 => {
                    if let (Some(compr), Some(orig)) = (read_u64(&data[0..8]), read_u64(&data[8..16])) {
                        compressed_size = compr;
                        original_size = orig;
                    }
                }
                _ => {}
            }
            parser.update_checksums_no_wrapping_sum(header);
            extra_header_len = if raw_header.lha_level == 3 {
                u32::from_le_bytes(*header.last_chunk::<4>().unwrap()) as usize
            }
            else {
                u16::from_le_bytes(*header.last_chunk::<2>().unwrap()) as usize
            }
        }

        // validate long header length
        if long_header_len != 0 &&
           long_header_len as usize != parser.len
        {
            if raw_header.lha_level == 2 && (long_header_len as usize) - 1 == parser.len
            {
                // read padding byte
                parser.read_u8()?;
            }
            else if raw_header.lha_level != 2 || long_header_len as usize != parser.len - 2
            {
                // some packers (Osk) don't include self in the header length
                return Err(LhaError::HeaderParse(LhaHeaderError::LongSizeMismatch))
            }
        }

        // validate headers CRC
        if let Some(crc) = header_crc && crc != parser.crc.sum16() {
            return Err(LhaError::HeaderParse(LhaHeaderError::Crc16Mismatch))
        }

        // adjust compressed size for level 1
        if raw_header.lha_level == 1 {
            compressed_size = compressed_size.checked_sub(extra_headers.len() as u64)
                .ok_or(LhaError::HeaderParse(LhaHeaderError::SkipSizeMismatch))?
        }

        let compression = raw_header.compression;
        let last_modified = u32::from_le_bytes(raw_header.last_modified);
        let extra_headers = extra_headers.into_boxed_slice();

        Ok(Some(LhaHeader {
            level: raw_header.lha_level,
            compression,
            compressed_size,
            original_size,
            filename,
            os_type,
            msdos_attrs,
            last_modified,
            file_crc,
            extended_area,
            first_header_len,
            extra_headers
        }))
    }

    /// Return an iterator that will iterate through extra headers, yielding the headers' raw
    /// data, excluding the next header length field.
    ///
    /// # Note
    /// Each iterated slice will have at least the size of 1 byte containing the header identifier.
    pub fn iter_extra(&self) -> ExtraHeaderIter<'_> {
        ExtraHeaderIter {
            data: &self.extra_headers,
            header_length: self.first_header_len,
            header_len32: self.level == 3
        }
    }
}

#[inline]
pub(super) fn read_u16(slice: &[u8]) -> Option<u16> {
    slice.as_array::<{size_of::<u16>()}>().copied().map(u16::from_le_bytes)
}

#[inline]
pub(super) fn read_u32(slice: &[u8]) -> Option<u32> {
    slice.as_array::<{size_of::<u32>()}>().copied().map(u32::from_le_bytes)
}

#[inline]
pub(super) fn read_u64(slice: &[u8]) -> Option<u64> {
    slice.as_array::<{size_of::<u64>()}>().copied().map(u64::from_le_bytes)
}

fn wrapping_csum(init: Wrapping<u8>, data: &[u8]) -> Wrapping<u8> {
    let sum: Wrapping<u8> = data.iter().copied().map(Wrapping).sum();
    sum + init
}

pub(super) fn split_data_at_nil_or_end(data: &[u8]) -> (&[u8], Option<&[u8]>) {
    match memchr::memchr(0, data) {
        Some(index) => {
            #[cfg(all(not(feature = "no-unsafe-assertions"), not(debug_assertions)))]
            unsafe {
                // SAFETY: memchr guarantee asserted condition
                core::hint::assert_unchecked(index < data.len());
            }
            (&data[0..index], Some(&data[index + 1..]))
        }
        None => (data, None)
    }
}

#[cfg(feature = "std")]
pub(super) fn parse_pathname(data: &[u8], path: &mut PathBuf) {
    path.reserve(data.len());
    // split by all possible path separators
    for part in data.split(|&c| matches!(c, 0xFF|b'/'|b'\\')) {
        match part {
            b"."|b".."|[] => {} // ignore malicious and empty paths
            name => path.push(parse_str_nilterm(name, false, false).as_ref())
        }
    }
}

pub(super) fn parse_pathname_to_str(data: &[u8], path: &mut String) {
    path.reserve(data.len());
    // split by all possible path separators
    for part in data.split(|&c| matches!(c, 0xFF|b'/'|b'\\')) {
        match part {
            b"."|b".."|[] => {} // ignore malicious and empty paths
            name => {
                if !path.is_empty() {
                    path.push('/');
                }
                path.push_str(parse_str_nilterm(name, false, false).as_ref())
            }
        }
    }
}

#[inline(always)]
pub(super) fn is_separator(c: char) -> bool {
    #[cfg(feature = "std")]
    {
        std::path::is_separator(c)
    }
    #[cfg(not(feature = "std"))]
    {
        matches!(c, '/'|'\\')
    }
}

pub(super) fn parse_str_nilterm(
        data: &[u8], nilterm: bool, ignore_sep: bool
    ) -> Cow<'_, str>
{
    if let Some(index) = data.iter().position(|&c|
            !(0x20..0x7f).contains(&c) ||
            (!ignore_sep && is_separator(c as char))
        )
    {
        let mut out = String::with_capacity(data.len()*3);
        let (head, rest) = data.split_at(index);
        // SAFETY: head was validated to contain ASCII-only characters
        out.push_str(unsafe {
            core::str::from_utf8_unchecked(head)
        });
        for byte in rest.iter() {
            match byte {
                0 if nilterm => break,
                0x00..=0x1f|
                0x7f..=0xff => {
                    write!(out, "%{:02x}", byte).unwrap();
                }
                &ch => {
                    let c = ch as char;
                    if !ignore_sep && is_separator(c) {
                        out.push('_');
                    }
                    else {
                        out.push(c);
                    }
                }
            }
        }
        Cow::Owned(out)
    }
    else {
        // SAFETY: data was validated to contain ASCII-only characters
        unsafe {
            Cow::Borrowed(core::str::from_utf8_unchecked(data))
        }
    }
}

#[cfg(feature = "std")]
#[cfg(test)]
mod tests {
    use super::*;
    use std::path::MAIN_SEPARATOR;

    fn parse_filename(data: &[u8]) -> Cow<'_, str> {
        parse_str_nilterm(data, false, false)
    }

   #[test]
    fn split_data_at_nil_or_end_works() {
        assert_eq!((&b"Foo"[..], None), split_data_at_nil_or_end(b"Foo"));
        assert_eq!((&b"Foo"[..], Some(&b"Bar"[..])), split_data_at_nil_or_end(b"Foo\x00Bar"));
        assert_eq!((&[][..], Some(&b"Bar"[..])), split_data_at_nil_or_end(b"\x00Bar"));
    }

   #[test]
    fn path_parser_works() {
        assert_eq!("", parse_filename(b""));
        assert_eq!("Hello World!", parse_filename(b"Hello World!"));
        if std::path::is_separator('/') {
            assert_eq!("_Hello_World_", parse_filename(b"/Hello/World/"));
        }
        if std::path::is_separator('\\') {
            assert_eq!("_Hello_World_", parse_filename(br"\Hello\World\"));
        }
        assert_eq!("Hello%00World%7f", parse_filename(b"Hello\x00World\x7f"));
        assert_eq!("Hello%01World%ff", parse_filename(b"Hello\x01World\xff"));
        assert_eq!("Hello", parse_str_nilterm(b"Hello\x00World\xff", true, false));
        if std::path::is_separator('/') {
            assert_eq!("He_llo", parse_str_nilterm(b"He/llo\x00World\xff", true, false));
            assert_eq!("He/llo", parse_str_nilterm(b"He/llo\x00World\xff", true, true));
            assert_eq!("He/llo%00World%ff", parse_str_nilterm(b"He/llo\x00World\xff", false, true));
            assert_eq!("_Hello%1fWorld%80", parse_filename(b"/Hello\x1fWorld\x80"));
        }
        let mut path = PathBuf::new();
        parse_pathname(b"", &mut path);
        assert!(path.is_relative());
        assert_eq!("", path.to_str().unwrap());
        parse_pathname(b"/", &mut path);
        assert!(path.is_relative());
        assert_eq!("", path.to_str().unwrap());
        parse_pathname(br"\", &mut path);
        assert!(path.is_relative());
        assert_eq!("", path.to_str().unwrap());
        parse_pathname(br".", &mut path);
        assert!(path.is_relative());
        assert_eq!("", path.to_str().unwrap());
        parse_pathname(br"..", &mut path);
        assert!(path.is_relative());
        assert_eq!("", path.to_str().unwrap());
        parse_pathname(br"./..", &mut path);
        assert!(path.is_relative());
        assert_eq!("", path.to_str().unwrap());
        parse_pathname(br".\..", &mut path);
        assert!(path.is_relative());
        assert_eq!("", path.to_str().unwrap());
        parse_pathname(br"/..\./", &mut path);
        assert!(path.is_relative());
        assert_eq!("", path.to_str().unwrap());
        parse_pathname(br"\../.\", &mut path);
        assert!(path.is_relative());
        assert_eq!("", path.to_str().unwrap());
        parse_pathname(br"foo/bar\baz", &mut path);
        assert!(path.is_relative());
        let expect = format!("foo{}bar{}baz", MAIN_SEPARATOR, MAIN_SEPARATOR);
        assert_eq!(expect, path.to_str().unwrap());
        path.clear();
        parse_pathname(br"\foo/bar\baz/", &mut path);
        assert!(path.is_relative());
        let expect = format!("foo{}bar{}baz", MAIN_SEPARATOR, MAIN_SEPARATOR);
        assert_eq!(expect, path.to_str().unwrap());
        path.clear();
        parse_pathname(br"/foo\bar/baz\", &mut path);
        assert!(path.is_relative());
        let expect = format!("foo{}bar{}baz", MAIN_SEPARATOR, MAIN_SEPARATOR);
        assert_eq!(expect, path.to_str().unwrap());
        path.clear();
        parse_pathname(b"foo\xffbar\xffbaz", &mut path);
        assert!(path.is_relative());
        let expect = format!("foo{}bar{}baz", MAIN_SEPARATOR, MAIN_SEPARATOR);
        assert_eq!(expect, path.to_str().unwrap());
        path.clear();
        parse_pathname(b"\xfffoo\xffb\x91ar\xffbaz\xff", &mut path);
        assert!(path.is_relative());
        let expect = format!("foo{}b%91ar{}baz", MAIN_SEPARATOR, MAIN_SEPARATOR);
        assert_eq!(expect, path.to_str().unwrap());
        path.clear();
    }

    #[test]
    fn path_parser_to_str_works() {
        let mut path = String::new();
        parse_pathname_to_str(b"", &mut path);
        assert!(!path.starts_with('/'));
        assert_eq!("", &path);
        parse_pathname_to_str(b"/", &mut path);
        assert!(!path.starts_with('/'));
        assert_eq!("", &path);
        parse_pathname_to_str(br"\", &mut path);
        assert!(!path.starts_with('/'));
        assert_eq!("", &path);
        parse_pathname_to_str(br".", &mut path);
        assert!(!path.starts_with('/'));
        assert_eq!("", &path);
        parse_pathname_to_str(br"..", &mut path);
        assert!(!path.starts_with('/'));
        assert_eq!("", &path);
        parse_pathname_to_str(br"./..", &mut path);
        assert!(!path.starts_with('/'));
        assert_eq!("", &path);
        parse_pathname_to_str(br".\..", &mut path);
        assert!(!path.starts_with('/'));
        assert_eq!("", &path);
        parse_pathname_to_str(br"/..\./", &mut path);
        assert!(!path.starts_with('/'));
        assert_eq!("", &path);
        parse_pathname_to_str(br"\../.\", &mut path);
        assert!(!path.starts_with('/'));
        assert_eq!("", &path);
        parse_pathname_to_str(br"foo/bar\baz", &mut path);
        assert!(!path.starts_with('/'));
        let expect = "foo/bar/baz";
        assert_eq!(expect, &path);
        path.clear();
        parse_pathname_to_str(br"\foo/bar\baz/", &mut path);
        assert!(!path.starts_with('/'));
        let expect = "foo/bar/baz";
        assert_eq!(expect, &path);
        path.clear();
        parse_pathname_to_str(br"/foo\bar/baz\", &mut path);
        assert!(!path.starts_with('/'));
        let expect = "foo/bar/baz";
        assert_eq!(expect, &path);
        path.clear();
        parse_pathname_to_str(b"foo\xffbar\xffbaz", &mut path);
        assert!(!path.starts_with('/'));
        let expect = "foo/bar/baz";
        assert_eq!(expect, &path);
        path.clear();
        parse_pathname_to_str(b"\xfffoo\xffb\x91ar\xffbaz\xff", &mut path);
        assert!(!path.starts_with('/'));
        let expect = "foo/b%91ar/baz";
        assert_eq!(expect, &path);
        path.clear();
    }
}