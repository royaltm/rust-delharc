//! # **LHA** header definitions.
use core::mem;
use core::convert::TryFrom;
use core::num::Wrapping;
use core::slice;
use std::io::{self, Read};

use chrono::prelude::*;

#[non_exhaustive]
#[derive(Debug,Clone,Copy,PartialEq,Eq)]
pub enum CompressionMethod {
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
    Lhd,
    Pm0,
    Pm1,
    Pm2
}

impl TryFrom<&[u8;5]> for CompressionMethod {
    type Error = &'static str;
    fn try_from(s: &[u8;5]) -> Result<Self, Self::Error> {
        Ok(match s {
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
            b"-lhd-" => CompressionMethod::Lhd,
            b"-pm0-" => CompressionMethod::Pm0,
            b"-pm1-" => CompressionMethod::Pm1,
            b"-pm2-" => CompressionMethod::Pm2,
            _ => return Err("unrecognized compression method")
        })
    }
}

#[derive(Debug, Clone)]
pub struct LhaHeader {
    pub level: u8, // currently only 0
    pub compression: CompressionMethod,
    pub compressed_size: u32,
    pub original_size: u32,
    pub pathname: Vec<u8>,
    pub last_modified: NaiveDateTime,
    pub crc: u16
}

impl Default for LhaHeader {
    fn default() -> Self {
        LhaHeader {
            level: 0,
            compression: CompressionMethod::Lh0,
            compressed_size: 0,
            original_size: 0,
            pathname: Vec::new(),
            last_modified: datetime_epoch(),
            crc: 0
        }
    }
}

#[derive(Clone, Copy, Debug, Default)]
#[repr(C)]
#[repr(packed)]
struct LhaLevel0Header {
    csum: u8,
    compression: [u8;5],
    compressed_size: [u8;4],
    original_size: [u8;4],
    last_modified: [u8;4],
    file_attr_msdos: u8,
    lha_level: u8,
    pathname_len: u8,
}

impl LhaHeader {
    pub fn read<R: Read>(mut rd: R) -> io::Result<Option<Self>> {
        let mut header_size: u8 = 0;
        rd.read_exact(slice::from_mut(&mut header_size))?;
        if header_size == 0 {
            return Ok(None)
        }

        let mut raw_head = LhaLevel0Header::default();
        rd.read_exact(unsafe { struct_slice_mut(&mut raw_head) })?;
        if raw_head.lha_level != 0 {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "wrong header level"))
        }
        let csum = wrapping_csum(0, unsafe { struct_slice_ref(&raw_head) });
        let pathname_len = raw_head.pathname_len as usize;
        let min_len = mem::size_of_val(&raw_head) + pathname_len;
        if (header_size as usize) < min_len {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "wrong header size"))
        }

        let mut pathname = Vec::with_capacity(pathname_len);
        if rd.by_ref().take(pathname_len as u64).read_to_end(&mut pathname)? != pathname_len {
            return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "file too short"))
        }
        let csum = wrapping_csum(csum, &pathname);

        let mut crc = [0u8;2];
        rd.read_exact(&mut crc)?;
        let mut csum = wrapping_csum(csum, &crc);

        let extra_remaining = header_size as usize - min_len;
        if extra_remaining != 0 {
            let mut extra = Vec::with_capacity(extra_remaining);
            if rd.by_ref().take(extra_remaining as u64).read_to_end(&mut extra)? != extra_remaining {
                return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "file too short"))
            }
            csum = wrapping_csum(csum, &extra);
        }

        if csum != raw_head.csum {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "invalid header level checksum"))
        }

        let compression = CompressionMethod::try_from(&raw_head.compression).map_err(|e|
            io::Error::new(io::ErrorKind::InvalidData, e)
        )?;
        let compressed_size = u32::from_le_bytes(raw_head.compressed_size);
        let original_size = u32::from_le_bytes(raw_head.original_size);
        let last_modified = parse_msdos_datetime(u32::from_le_bytes(raw_head.last_modified));
        let crc = u16::from_le_bytes(crc);

        Ok(Some(LhaHeader {
            level: 0,
            compression,
            compressed_size,
            original_size,
            pathname,
            last_modified,
            crc
        }))
    }
}

fn wrapping_csum(init: u8, data: &[u8]) -> u8 {
    let sum: Wrapping<u8> = data.iter().copied().map(Wrapping).sum();
    (sum + Wrapping(init)).0
}

fn parse_msdos_datetime(ts: u32) -> NaiveDateTime {
    let sec = ts << 1 & 0x3e;
    let min = ts >> 5 & 0x3f;
    let hour = ts >> 11 & 0x1f;
    let day = ts >> 16 & 0x1f;
    let mon = ts >> 21 & 0xf;
    let year = 1980 + (ts >> 25 & 0x7f) as i32;
    NaiveDate::from_ymd_opt(year, mon, day).and_then(|d| d.and_hms_opt(hour, min, sec))
              .unwrap_or_else(|| datetime_epoch())
}

fn datetime_epoch() -> NaiveDateTime {
    NaiveDate::from_ymd(1980, 1, 1).and_hms(0, 0, 0)
}


/// # Safety
/// This function can be used safely only with packed structs that solely consist of
/// `u8` or array of `u8` primitives.
unsafe fn struct_slice_mut<T: Copy>(obj: &mut T) -> &mut [u8] {
    let len = core::mem::size_of::<T>() / core::mem::size_of::<u8>();
    core::slice::from_raw_parts_mut(obj as *mut T as *mut u8, len)
}

/// # Safety
/// This function can be used safely only with packed structs that solely consist of
/// `u8` or array of `u8` primitives.
unsafe fn struct_slice_ref<T: Copy>(obj: &T) -> &[u8] {
    let len = core::mem::size_of::<T>() / core::mem::size_of::<u8>();
    core::slice::from_raw_parts(obj as *const T as *const u8, len)
}
