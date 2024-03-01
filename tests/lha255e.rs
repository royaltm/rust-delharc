use std::{io::{self, Seek, SeekFrom}, fs};
use delharc::header::*;

mod sink;
use sink::SinkSum;

const TESTS_CASES: &[(u64, &str, &str, u64, u64, u16, u32, &str, u8, CompressionMethod)] = &[
    (0x6A6, "sfx.exe", "GPL-2",      7004,   18092, 0xA33A, 0x4E46F4A1, "2010-01-01 00:00:00", 0, CompressionMethod::Lh5),
    (0, "lh0.lzh",      "GPL-2.GZ",   6829,    6829, 0xB6D5, 0xE4690583, "2010-01-01 00:00:00", 1, CompressionMethod::Lh0),
    (0, "lh5.lzh",      "GPL-2",      7004,   18092, 0xA33A, 0x4E46F4A1, "2010-01-01 00:00:00", 1, CompressionMethod::Lh5),
    (0, "subdir.lzh",
            "SUBDIR*SUBDIR2*HELLO.TXT", 12,      12, 0x9778, 0xAF083B2D, "2010-01-01 00:00:00", 1, CompressionMethod::Lh0),
];

#[test]
fn test_lha255e() -> io::Result<()> {
    for (offset, name, path, size_c, size_o, crc16, crc32, modif, level, compr) in TESTS_CASES {
        println!("-------------\n{:?}", name);
        let mut file = fs::File::open(format!("tests/lha255e/{}", name))?;
        file.seek(SeekFrom::Start(*offset))?;
        let mut lha_reader = delharc::LhaDecodeReader::new(file)?;
        loop {
            let mut sink = SinkSum::new();
            let header = lha_reader.header();
            assert_eq!(header.level, *level);
            assert_eq!(header.msdos_attrs, MsDosAttrs::ARCHIVE);
            let path1 = path.replace("*", &std::path::MAIN_SEPARATOR.to_string());
            assert_eq!(&header.parse_pathname().to_str().unwrap(), &path1);
            let path1 = path.replace("*", "/");
            assert_eq!(&header.parse_pathname_to_str(), &path1);
            if *level == 0 {
                assert_eq!(header.parse_os_type()?, OsType::Generic);
            }
            else {
                assert_eq!(header.parse_os_type()?, OsType::MsDos);
            }
            assert_eq!(header.compression_method().unwrap(), *compr);
            assert_eq!(header.compressed_size, *size_c);
            assert_eq!(header.original_size, *size_o);
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
