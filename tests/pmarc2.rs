#![cfg(all(feature = "std", feature = "pm"))]
use std::{io::{self, Seek, SeekFrom}, fs};
use delharc::header::*;

mod sink;
use sink::SinkSum;
use CompressionMethod::*;

const TESTS_CASES: &[(u64, &str, &str, u64, u64, u16, u32, OsType, CompressionMethod)] = &[
    (0, "pm0.pma",     "GPL-2.GZ",    6912,    6912, 0x39CC, 0x549D935A, OsType::Generic, Pm0),
    (0, "pm2.pma",     "GPL-2.",      7098,   18176, 0x83CD, 0x8E2093A7, OsType::Generic, Pm2),
    (0, "long.pma",    "LONG.TXT",   85397, 1241659, 0x2AEA, 0xB2B419D6, OsType::Generic, Pm2),
    (0, "comment.pma", "HELLO.TXT",     22,     128, 0x9784, 0x906330C5, OsType::TownsOs, Pm2),
    (0x86D, "sfx.com", "GPL-2.",      7098,   18176, 0x83cd, 0x8E2093A7, OsType::Generic, Pm2),
];

const EXTENDED_COMMENT: &str = "his is a comment attached to the file hello.txt.";

#[test]
fn test_pmarc2() -> io::Result<()> {
    for (offset, name, path, size_c, size_o, crc16, crc32, ostype, compr) in TESTS_CASES {
        println!("-------------\n{:?}", name);
        let mut file = fs::File::open(format!("tests/pmarc2/{}", name))?;
        file.seek(SeekFrom::Start(*offset))?;
        let mut lha_reader = delharc::LhaDecodeReader::new(file)?;
        loop {
            let mut sink = SinkSum::new();
            let header = lha_reader.header();
            if !header.extended_area.is_empty() {
                assert_eq!(str::from_utf8(&header.extended_area).unwrap(), EXTENDED_COMMENT);
            }
            assert_eq!(header.level, 0);
            assert_eq!(header.msdos_attrs, MsDosAttrs::ARCHIVE);
            let path1 = path.replace("*", &std::path::MAIN_SEPARATOR.to_string());
            assert_eq!(&header.parse_pathname().to_str().unwrap(), &path1);
            let path1 = path.replace("*", "/");
            assert_eq!(&header.parse_pathname_to_str(), &path1);
            assert_eq!(header.parse_os_type()?, *ostype);
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
