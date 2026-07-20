#![cfg(feature = "std")]
use std::{fs, io::{self, Seek, SeekFrom}};
use delharc::header::*;

mod sink;
use sink::SinkSum;

use CompressionMethod::*;
const TESTS_CASES: &[(u64, &str, &str, u64, u64, u16, u32, &str, &str, u8, CompressionMethod)] = &[
    (0, "lh0.lzh", "gpl-2.gz",          6829,  6829, 0xB6D5, 0xE4690583, "-----", "2009-12-31 20:00:00", 0, Lh0),
    (0, "lh5.lzh", "gpl-2",             6996, 18092, 0xA33A, 0x4E46F4A1, "-----", "2009-12-31 20:00:00", 0, Lh5),
    (0, "readonly.lzh", "readonly.txt",   12,    12, 0x5406, 0x3C38C801, "A---R", "2025-06-28 12:27:42", 0, Lh0),
    (0, "subdir.lzh",
           "subdir*SUBDIR2*HELLO.TXT",    14,    14, 0xF1C6, 0xB0595A59, "-----", "2025-06-28 12:02:02", 0, Lh0),
    (0x59b1, "sfx.exe", "gpl-2.gz",     6829,  6829, 0xB6D5, 0xE4690583, "-----", "2009-12-31 20:00:00", 0, Lh0),
];

const EA_CASES: &[(&str, &[(&str, u64, u64, u16, u32, &str, &str, u8, CompressionMethod)])] = &[
    ("eas.lzh", &[
        ("EAS*hello.txt", 294, 420, 0x8820, 0x368B0EEB, "-----", "2025-06-28 12:12:42", 1, CompressionMethod::Lh5),
        ("hello.txt",      14,  14, 0xF1C6, 0xB0595A59, "-----", "2025-06-28 12:06:42", 0, CompressionMethod::Lh0),
        ("Apply-Ea.Cmd",  292, 505, 0x9118, 0x801036A8, "-----", "2025-06-28 12:12:42", 0, CompressionMethod::Lh5),
    ]),
    ("easubdir.lzh", &[
        ("subdir",        354, 570, 0x9B49, 0x69D04C82, "-D---", "2025-06-28 12:30:12", 0, Lh5),
        ("subdir*subdir2*hello.txt",
                           14,  14, 0xF1C6, 0xB0595A59, "-----", "2025-06-28 12:14:58", 0, Lh0),
        ("Apply-Ea.Cmd",  277, 486, 0x1961, 0x72A16878, "-----", "2025-06-28 12:30:12", 0, Lh5),
    ]),
];

#[test]
fn test_lha2_222() -> io::Result<()> {
    for (offset, name, path, size_c, size_o, crc16, crc32, attr, modif, level, compr) in TESTS_CASES {
        println!("-------------\n{:?}", name);
        let pathname = format!("tests/lh2_222/{}", name);
        let mut lha_reader = if *offset == 0 {
            delharc::parse_file(pathname)?
        }
        else {
            let mut lha_reader: delharc::LhaDecodeReader::<fs::File> = Default::default();
            let mut file = fs::File::open(pathname)?;
            file.seek(SeekFrom::Start(*offset))?;
            assert!(lha_reader.begin_new(file)?);
            lha_reader
        };
        for filen in 0.. {
            assert!(filen <= 0);
            let mut sink = SinkSum::new();
            let header = lha_reader.header();
            assert_eq!(header.level, *level);
            if header.level == 0 {
                assert_eq!(header.parse_os_type()?, OsType::Generic);
            }
            else {
                assert_eq!(header.parse_os_type()?, OsType::MsDos);
            }
            assert!(!header.is_directory());
            assert_eq!(header.msdos_attrs.to_string(), *attr);
            assert_eq!(header.compression_method().unwrap(), *compr);
            assert_eq!(header.compressed_size, *size_c);
            assert_eq!(header.original_size, *size_o);
            let path1 = path.replace("*", &std::path::MAIN_SEPARATOR.to_string());
            assert_eq!(&header.parse_pathname().to_str().unwrap(), &path1);
            let path1 = path.replace("*", "/");
            assert_eq!(&header.parse_pathname_to_str(), &path1);
            assert!(header.parse_comment().is_none());
            let last_modified = format!("{}", header.parse_last_modified());
            assert_eq!(&last_modified, modif);
            assert!(header.parse_os_9_attrs().is_none());
            assert!(header.parse_unix_permissions().is_none());
            assert!(header.parse_unix_uid_gid().is_none());
            assert_eq!(header.file_crc, *crc16);
            io::copy(&mut lha_reader, &mut sink)?;
            assert_eq!(sink.length, *size_o as u64);
            assert_eq!(sink.crc32.get_crc(), *crc32);
            assert_eq!(sink.crc16.get_crc(), *crc16);
            assert_eq!(lha_reader.crc_check().unwrap(), *crc16);
            if !lha_reader.seek_next_file().unwrap() {
                break;
            }
        }
    }

    for (name, headers) in EA_CASES {
        println!("-------------\n{:?}", name);
        let mut lha_reader = delharc::parse_file(format!("tests/lh2_222/{}", name))?;
        for filen in 0.. {
            assert!(filen < headers.len());
            let (path, size_c, size_o, crc16, crc32, attr, modif, level, compr) = &headers[filen];
            let mut sink = SinkSum::new();
            let header = lha_reader.header();
            assert_eq!(header.level, *level);
            if header.level == 0 {
                assert_eq!(header.parse_os_type()?, OsType::Generic);
            }
            else {
                assert_eq!(header.parse_os_type()?, OsType::MsDos);
            }
            assert_eq!(header.msdos_attrs.to_string(), *attr);
            assert_eq!(header.compression_method().unwrap(), *compr);
            assert_eq!(header.compressed_size, *size_c);
            assert_eq!(header.original_size, *size_o);
            let path1 = path.replace("*", &std::path::MAIN_SEPARATOR.to_string());
            assert_eq!(&header.parse_pathname().to_str().unwrap(), &path1);
            let path1 = path.replace("*", "/");
            assert_eq!(&header.parse_pathname_to_str(), &path1);
            let last_modified = format!("{}", header.parse_last_modified());
            assert_eq!(&last_modified, modif);
            assert_eq!(header.file_crc, *crc16);
            io::copy(&mut lha_reader, &mut sink)?;
            assert_eq!(sink.length, *size_o as u64);
            assert_eq!(sink.crc32.get_crc(), *crc32);
            assert_eq!(sink.crc16.get_crc(), *crc16);
            assert_eq!(lha_reader.crc_check().unwrap(), *crc16);
            if !lha_reader.next_file().unwrap() {
                break;
            }
        }
    }
    Ok(())
}
