#![cfg(all(feature = "std", feature = "lhx"))]
use std::io;
use delharc::header::*;

mod sink;
use sink::SinkSum;

const TESTS_CASES: &[(&str, &str, u64, u64, u16, u32, &str, u8, CompressionMethod)] = &[
    ("h2_lhx.lzh",      "GPL-2",   6828,   18092, 0xA33A, 0x4E46F4A1, "2013-07-24 13:41:40 UTC", 2, CompressionMethod::Lhx),
    ("lhx_long.lzh", "long.txt",  77318, 1241658, 0x6a7c, 0x06788E85, "2013-07-24 13:42:20 UTC", 2, CompressionMethod::Lhx),
];

const UNSUPPORTED_CASES: &[(&str, &str, u64, u64, u16, &str, u8, [u8;5])] = &[
    ("lh2.lzh", "LICENSE.MIT", 1296, 2853, 0xD157, "2021-11-10 16:08:59.207157300 UTC", 2, *b"-lh2-"),
    ("lh3.lzh", "LICENSE.MIT", 1337, 2853, 0xD157, "2021-11-10 16:08:59.207157300 UTC", 2, *b"-lh3-"),
];

#[test]
fn test_unlha32() -> io::Result<()> {
    for (name, path, size_c, size_o, crc16, crc32, modif, level, compr) in TESTS_CASES {
        println!("-------------\n{:?}", name);
        let mut lha_reader = delharc::parse_file(format!("tests/unlha32/{}", name))?;
        let mut sink = SinkSum::new();
        let header = lha_reader.header();
        assert_eq!(header.level, *level);
        assert!(!header.is_directory());
        let path = path.replace("*", &std::path::MAIN_SEPARATOR.to_string());
        assert_eq!(header.msdos_attrs, MsDosAttrs::ARCHIVE);
        assert_eq!(header.compression_method().unwrap(), *compr);
        assert_eq!(header.compressed_size, *size_c);
        assert_eq!(header.original_size, *size_o);
        assert_eq!(&header.parse_pathname_to_str(), &path);
        assert!(header.parse_os_9_attrs().is_none());
        assert!(header.parse_unix_permissions().is_none());
        assert!(header.parse_unix_uid_gid().is_none());
        assert!(header.parse_comment().is_none());
        let last_modified = format!("{}", header.parse_last_modified());
        assert_eq!(&last_modified, modif);
        assert_eq!(header.file_crc, *crc16);
        assert_eq!(header.parse_os_type()?, OsType::MsDos);
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
        assert!(!lha_reader.next_file().unwrap());
    }
    Ok(())
}

#[test]
fn test_unlha32_unsupported() -> io::Result<()> {
    for (name, path, size_c, size_o, crc16, modif, level, compr) in UNSUPPORTED_CASES {
        println!("-------------\n{:?}", name);
        let mut lha_reader = delharc::parse_file(format!("tests/unlha32/{}", name))?;
        let mut sink = SinkSum::new();
        let header = lha_reader.header();
        assert_eq!(header.level, *level);
        assert!(!header.is_directory());
        let path = path.replace("*", &std::path::MAIN_SEPARATOR.to_string());
        assert_eq!(header.msdos_attrs, MsDosAttrs::ARCHIVE);
        assert_eq!(&header.compression, compr);
        assert_eq!(header.compression_method().unwrap_err(), UnrecognizedCompressionMethod(*compr));
        assert_eq!(header.compressed_size, *size_c);
        assert_eq!(header.original_size, *size_o);
        assert_eq!(&header.parse_pathname_to_str(), &path);
        assert!(header.parse_os_9_attrs().is_none());
        assert!(header.parse_unix_permissions().is_none());
        assert!(header.parse_unix_uid_gid().is_none());
        assert!(header.parse_comment().is_none());
        let last_modified = format!("{}", header.parse_last_modified());
        assert_eq!(&last_modified, modif);
        assert_eq!(header.file_crc, *crc16);
        assert_eq!(header.parse_os_type()?, OsType::MsDos);
        assert!(io::copy(&mut lha_reader, &mut sink).is_err());
        assert!(!lha_reader.seek_next_file().unwrap());
    }
    Ok(())
}
