//! This is an example program using the delharc library.
//!
//! This program lists all files contained in an archive file.
//!
//! This program expects arguments - paths to the archive files.
//!
//! This program runs only with `std` feature enabled.
#[cfg(feature = "std")]
use core::fmt::{self, Write};
#[cfg(feature = "std")]
use std::{env, fs, io};
#[cfg(feature = "std")]
use delharc::*;

#[cfg(feature = "std")]
fn list_files<R: io::Read + io::Seek>(file: R) -> io::Result<()> {
    let mut lha_reader = LhaDecodeReader::new(file)?;
    println!("L|Compressed| Original |Compr| System | Att/Perm |       Date/Time       | File path");
    println!("=|==========|==========|=====|========|==========|=======================|==============");
    let mut date_time = String::with_capacity(23);
    loop {
        let header = lha_reader.header();
        let filename = header.parse_pathname_to_str();
        // let compression = header.compression_method().ok();
        let os_type = header.parse_os_type()?;
        let perm = header.parse_unix_permissions();
        let os9_attr = header.parse_os_9_attrs();
        let attr_perm: &dyn fmt::Display = match os_type {
            OsType::Unix if let Some(perm) = perm.as_ref() => {
                perm as _
            }
            OsType::Os9|OsType::Osk if let Some(os9_attr) = os9_attr.as_ref() => {
                os9_attr as _
            }
            _ => &header.msdos_attrs as _
        };
        let comment = header.parse_comment();
        // let (uid, gid) = header.parse_unix_uid_gid().unwrap_or((u16::MAX, u16::MAX));
        let dt = header.parse_last_modified();
        date_time.clear();
        write!(date_time, "{}", dt).unwrap();
        print!("{}|{:10}|{:10}|{}|{:^8}|{:^10}|{:^23}|{}",
            header.level,
            header.compressed_size,
            header.original_size,
            str::from_utf8(&header.compression).unwrap_or("???"),
            os_type,
            attr_perm,
            date_time,
            filename);
        if let Some(comment) = comment {
            print!(" ({})", comment);
        }
        println!();
        // println!(" CRC={:#06X}", header.file_crc);
        if !lha_reader.seek_next_file()? {
            break
        }
    }
    Ok(())
}

#[cfg(feature = "std")]
fn main() -> io::Result<()> {
    let mut listed = false;
    for file_name in env::args().skip(1) {
        println!("Archive: {}\r\n{}", file_name, "=".repeat(88));
        let file = fs::File::open(file_name)?;
        list_files(file)?;
        println!("{}\r\n", "-".repeat(88));
        listed = true;
    }
    if !listed {
        eprintln!("Nothing to list, expected archive path arguments!");
    }
    Ok(())
}

#[cfg(not(feature = "std"))]
fn main() {
    panic!("This program requires std feature enabled");
}
