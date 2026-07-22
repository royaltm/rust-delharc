#![cfg(all(feature = "std", feature = "pm"))]
use std::{io, fs};
use delharc::header::*;

mod sink;
use sink::SinkSum;

const TESTS_CASES: &[(&str, &str, u64, u64, u16, u32, CompressionMethod)] = &[
    ("pm0.pma",      "GPL-2.GZ",   6912,    6912, 0x978D, 0xE3CF61D2, CompressionMethod::Pm0),
    ("pm1.pma",   "COPYING.TXT",   9829,   25284, 0xE582, 0xB7D0E42C, CompressionMethod::Pm1),
    ("pm1_long.pma", "LONG.TXT", 105071, 1241659, 0x2AEA, 0xB2B419D6, CompressionMethod::Pm1),
];

const SPECIAL_FILE: &str = "mtcd.pma";
const SPECIAL_CASES: &[(&str, u64, u64, u16, u32, CompressionMethod)] = &[
    ("MTCD.DOC", 759, 1403, 0x42CA, 0xCEEC9F76, CompressionMethod::Pm1),
    ("CD.MTC",   197,  256, 0x71C8, 0x9206AB18, CompressionMethod::Pm1),
];

#[test]
fn test_pmarc124() -> io::Result<()> {
    for (name, path, size_c, size_o, crc16, crc32, compr) in TESTS_CASES {
        println!("-------------\n{:?}", name);
        let file = fs::File::open(format!("tests/pmarc124/{}", name))?;
        let mut lha_reader = delharc::LhaDecodeReader::new(file)?;
        loop {
            let mut sink = SinkSum::new();
            let header = lha_reader.header();
            assert_eq!(header.level, 0);
            assert!(!header.is_directory());
            assert_eq!(header.msdos_attrs, MsDosAttrs::ARCHIVE);
            let path1 = path.replace("*", &std::path::MAIN_SEPARATOR.to_string());
            assert_eq!(&header.parse_pathname().to_str().unwrap(), &path1);
            let path1 = path.replace("*", "/");
            assert_eq!(&header.parse_pathname_to_str(), &path1);
            assert_eq!(header.parse_os_type()?, OsType::Generic);
            assert_eq!(header.compression_method().unwrap(), *compr);
            assert_eq!(header.compressed_size, *size_c);
            assert_eq!(header.original_size, *size_o);
            let last_modified = format!("{}", header.parse_last_modified());
            assert_eq!(&last_modified, "-");
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

#[test]
fn test_pmarc124_special() -> io::Result<()> {
    let name = SPECIAL_FILE;
    let file = fs::File::open(format!("tests/pmarc124/{}", name))?;
    let mut lha_reader = delharc::LhaDecodeReader::new(file)?;
    println!("-------------\n{:?}", name);
    for (i, (path, size_c, size_o, crc16, crc32, compr)) in SPECIAL_CASES.iter().enumerate() {
        let mut sink = SinkSum::new();
        let header = lha_reader.header();
        assert_eq!(header.level, 0);
        assert!(!header.is_directory());
        let path1 = path.replace("*", &std::path::MAIN_SEPARATOR.to_string());
        assert_eq!(&header.parse_pathname().to_str().unwrap(), &path1);
        let path1 = path.replace("*", "/");
        assert_eq!(&header.parse_pathname_to_str(), &path1);
        assert_eq!(header.parse_os_type()?, OsType::Generic);
        assert_eq!(header.compression_method().unwrap(), *compr);
        assert_eq!(header.compressed_size, *size_c);
        assert_eq!(header.original_size, *size_o);
        let last_modified = format!("{}", header.parse_last_modified());
        assert_eq!(&last_modified, "-");
        assert_eq!(header.file_crc, *crc16);
        io::copy(&mut lha_reader, &mut sink)?;
        assert_eq!(sink.length, *size_o as u64);
        assert_eq!(sink.crc32.get_crc(), *crc32);
        assert_eq!(sink.crc16.get_crc(), *crc16);
        assert_eq!(lha_reader.crc_check().unwrap(), *crc16);
        assert_eq!(lha_reader.next_file().unwrap(), i < SPECIAL_CASES.len() - 1);
    }
    Ok(())
}
