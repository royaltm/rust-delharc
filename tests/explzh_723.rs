#![cfg(feature = "std")]
use std::{fs, io::{self, Seek, SeekFrom}};
use delharc::header::*;

mod sink;
use sink::SinkSum;

use CompressionMethod::*;

const TESTS_CASES: &[(u64, &str, &str, u64, u64, u16, u32, &str, u8, CompressionMethod)] = &[
    (0, "h0_lh0.lzh", "gpl-2.gz", 6829,   6829, 0xB6D5, 0xE4690583, "2010-01-01 01:00:00",     0, Lh0),
    (0, "h0_lh5.lzh", "gpl-2",    6996,  18092, 0xA33A, 0x4E46F4A1, "2010-01-01 01:00:00",     0, Lh5),
    (0, "h0_lh6.lzh", "gpl-2",    6832,  18092, 0xA33A, 0x4E46F4A1, "2010-01-01 01:00:00",     0, Lh6),
    (0, "h0_lh7.lzh", "gpl-2",    6832,  18092, 0xA33A, 0x4E46F4A1, "2010-01-01 01:00:00",     0, Lh7),
    (0, "h1_lh0.lzh", "gpl-2.gz", 6829,   6829, 0xB6D5, 0xE4690583, "2010-01-01 01:00:00",     1, Lh0),
    (0, "h1_lh5.lzh", "gpl-2",    6996,  18092, 0xA33A, 0x4E46F4A1, "2010-01-01 01:00:00",     1, Lh5),
    (0, "h1_lh6.lzh", "gpl-2",    6832,  18092, 0xA33A, 0x4E46F4A1, "2010-01-01 01:00:00",     1, Lh6),
    (0, "h1_lh7.lzh", "gpl-2",    6832,  18092, 0xA33A, 0x4E46F4A1, "2010-01-01 01:00:00",     1, Lh7),
    (0, "h2_lh0.lzh", "gpl-2.gz", 6829,   6829, 0xB6D5, 0xE4690583, "2010-01-01 05:00:00 UTC", 2, Lh0),
    (0, "h2_lh5.lzh", "gpl-2",    6996,  18092, 0xA33A, 0x4E46F4A1, "2010-01-01 05:00:00 UTC", 2, Lh5),
    (0, "h2_lh6.lzh", "gpl-2",    6832,  18092, 0xA33A, 0x4E46F4A1, "2010-01-01 05:00:00 UTC", 2, Lh6),
    (0, "h2_lh7.lzh", "gpl-2",    6832,  18092, 0xA33A, 0x4E46F4A1, "2010-01-01 05:00:00 UTC", 2, Lh7),
    (0x8E00,  "declha_sfx_ansi.exe",    "gpl-2", 6996, 18092, 0xA33A, 0x4E46F4A1, "2010-01-01 01:00:00", 1, Lh5),
    (0x103E4, "declha_sfx_unicode.exe", "gpl-2", 6996, 18092, 0xA33A, 0x4E46F4A1, "2010-01-01 01:00:00", 1, Lh5),
];

const SUBDIR_CASES: &[(&str, &[(&str, u64, u64, u16, u32, &str, u8, CompressionMethod)])] = &[
    ("h0_subdir.lzh", &[
        ("subdir*",                   0,   0, 0x0000, 0x00000000, "2023-07-16 21:07:16", 0, Lhd),
        ("subdir*subdir2*",           0,   0, 0x0000, 0x00000000, "2023-07-16 21:07:16", 0, Lhd),
        ("subdir*subdir2*hello.txt", 12,  12, 0x9778, 0xAF083B2D, "2010-01-01 01:00:00", 0, Lh0)]),
    ("h1_subdir.lzh", &[
        ("subdir*",                   0,   0, 0x0000, 0x00000000, "2023-07-16 21:07:16", 1, Lhd),
        ("subdir*subdir2*",           0,   0, 0x0000, 0x00000000, "2023-07-16 21:07:16", 1, Lhd),
        ("subdir*subdir2*hello.txt", 12,  12, 0x9778, 0xAF083B2D, "2010-01-01 01:00:00", 1, Lh0)]),
    ("h2_subdir.lzh", &[
        ("subdir*",                   0,   0, 0x0000, 0x00000000, "2023-07-17 01:07:17.184287900 UTC", 2, Lhd),
        ("subdir*subdir2*",           0,   0, 0x0000, 0x00000000, "2023-07-17 01:07:17.184287900 UTC", 2, Lhd),
        ("subdir*subdir2*hello.txt", 12,  12, 0x9778, 0xAF083B2D, "2010-01-01 05:00:00 UTC", 2, Lh0)]),
];

#[test]
fn test_explzh_723() -> io::Result<()> {
    for (offset, name, path, size_c, size_o, crc16, crc32, modif, level, compr) in TESTS_CASES {
        println!("-------------\n{:?}", name);
        let mut lha_reader = if *offset == 0 {
            delharc::parse_file(format!("tests/explzh_723/{}", name))?
        }
        else {
            let mut lha_reader: delharc::LhaDecodeReader::<fs::File> = Default::default();
            let mut file = fs::File::open(format!("tests/explzh_723/{}", name))?;
            file.seek(SeekFrom::Start(*offset))?;
            assert!(lha_reader.begin_new(file)?);
            lha_reader
        };
        for filen in 0.. {
            assert!(filen <= 0);
            let mut sink = SinkSum::new();
            let header = lha_reader.header();
            // println!("{:?}", header.parse_pathname());
            // println!("{:?}", header.parse_pathname_to_str());
            // println!("{:?}", header);
            // for extra in header.iter_extra() {
            //     println!("{:02x?}", extra);
            // }
            assert_eq!(header.level, *level);
            if header.compression_method().unwrap() == CompressionMethod::Lhd {
                assert_eq!(header.msdos_attrs, MsDosAttrs::SUBDIR);
            }
            else {
                assert_eq!(header.msdos_attrs, MsDosAttrs::ARCHIVE);
            }
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
            if header.level == 0 {
                assert_eq!(header.parse_os_type()?, OsType::Generic);
            }
            else {
                assert_eq!(header.parse_os_type()?, OsType::MsDos);
            }
            assert!(header.parse_os_9_attrs().is_none());
            assert!(header.parse_unix_permissions().is_none());
            assert!(header.parse_unix_uid_gid().is_none());
            if *compr == CompressionMethod::Lhd {
                assert!(io::copy(&mut lha_reader, &mut sink).is_err());
            }
            else {
                io::copy(&mut lha_reader, &mut sink)?;
            }
            assert_eq!(sink.length, *size_o as u64);
            assert_eq!(sink.crc32.get_crc(), *crc32);
            assert_eq!(sink.crc16.get_crc(), *crc16);
            assert_eq!(lha_reader.crc_check().unwrap(), *crc16);
            if !lha_reader.seek_next_file().unwrap() {
                break;
            }
        }
    }

    for (name, headers) in SUBDIR_CASES {
        println!("-------------\n{:?}", name);
        let mut lha_reader = delharc::parse_file(format!("tests/explzh_723/{}", name))?;
        for filen in 0.. {
            assert!(filen < headers.len());
            let (path, size_c, size_o, crc16, crc32, modif, level, compr) = &headers[filen];
            let mut sink = SinkSum::new();
            let header = lha_reader.header();
            assert_eq!(header.level, *level);
            if header.compression_method().unwrap() == CompressionMethod::Lhd {
                assert_eq!(header.msdos_attrs, MsDosAttrs::SUBDIR);
            }
            else {
                assert_eq!(header.msdos_attrs, MsDosAttrs::ARCHIVE);
            }
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
            if header.level == 0 {
                assert_eq!(header.parse_os_type()?, OsType::Generic);
            }
            else {
                assert_eq!(header.parse_os_type()?, OsType::MsDos);
            }
            assert!(header.parse_os_9_attrs().is_none());
            assert!(header.parse_unix_permissions().is_none());
            assert!(header.parse_unix_uid_gid().is_none());
            if *compr == CompressionMethod::Lhd {
                assert!(io::copy(&mut lha_reader, &mut sink).is_err());
            }
            else {
                io::copy(&mut lha_reader, &mut sink)?;
            }
            assert_eq!(sink.length, *size_o as u64);
            assert_eq!(sink.crc32.get_crc(), *crc32);
            assert_eq!(sink.crc16.get_crc(), *crc16);
            assert_eq!(lha_reader.crc_check().unwrap(), *crc16);
            if !lha_reader.seek_next_file().unwrap() {
                break;
            }
        }
    }
    Ok(())
}
