#[cfg(feature = "std")]
use core::fmt::{self, Write};
#[cfg(feature = "std")]
use std::{env, fs, io};
#[cfg(feature = "std")]
use delharc::*;

#[cfg(feature = "std")]
fn list_files<R: io::Read>(file: R) -> io::Result<()> {
    let mut lha_reader = LhaDecodeReader::new(file)?;
    println!("L|Compressed| Original |Compr| OS/ Perm |       Date/Time       | File path");
    println!("=|==========|==========|=====|==========|=======================|==============");
    let mut date_time = String::with_capacity(23);
    loop {
        let header = lha_reader.header();
        let filename = header.parse_pathname_to_str();
        let compression = header.compression_method()?;
        let os_type = header.parse_os_type()?;
        let perm = header.parse_unix_permissions();
        let os_perm: &dyn fmt::Display = if let Some(perm) = perm.as_ref() {
            perm as _
        }
        else if matches!(os_type, OsType::Generic|OsType::MsDos) {
            &header.msdos_attrs as _
        }
        else {
            &os_type as _
        };
        let comment = header.parse_comment();
        // let (uid, gid) = header.parse_unix_uid_gid().unwrap_or((u16::MAX, u16::MAX));
        let dt = header.parse_last_modified();
        date_time.clear();
        write!(date_time, "{}", dt).unwrap();
        print!("{}|{:10}|{:10}|{}|{:^10}|{:^23}|{}",
            header.level,
            header.compressed_size,
            header.original_size,
            compression,
            os_perm,
            date_time,
            filename);
        if let Some(comment) = comment {
            print!(" ({})", comment);
        }
        println!();
        if !lha_reader.next_file()? {
            break
        }
    }
    Ok(())
}

#[cfg(feature = "std")]
fn main() -> io::Result<()> {
    let file_name = env::args().skip(1).next().ok_or_else(|| io::Error::other("missing archive file name argument!"))?;
    println!("Archive: {}", file_name);
    let file = fs::File::open(file_name)?;
    list_files(file)
}

#[cfg(not(feature = "std"))]
fn main() {
    panic!("This program requires std feature enabled");
}
