use std::io;
use delharc::header::*;

mod sink;
use sink::SinkSum;

const TESTS_CASES: &[(&str, &str, u64, u64, u16, u32, &str, u8, CompressionMethod)] = &[
    #[cfg(feature = "lh1")]
    ("lh1.lzh",    "gpl-2",    7201, 18092, 0xA33A, 0x4E46F4A1, "1980-06-12 21:03:18", 0, CompressionMethod::Lh1),
    ("lh6.lzh",    "gpl-2",    6832, 18092, 0xA33A, 0x4E46F4A1, "1980-06-12 21:03:18", 0, CompressionMethod::Lh6),
    ("level0.lzh",
        "subdir*subdir2*hello.txt", 12, 12, 0x9778, 0xAF083B2D, "1980-06-12 21:06:54", 0, CompressionMethod::Lh0),
    ("level1.lzh",
        "subdir*subdir2*hello.txt", 12, 12, 0x9778, 0xAF083B2D, "1980-06-12 21:06:54", 1, CompressionMethod::Lh0),
    ("level2.lzh",
        "subdir*subdir2*hello.txt", 12, 12, 0x9778, 0xAF083B2D, "1980-06-12 21:06:54 UTC", 2, CompressionMethod::Lh0),
];

#[test]
fn test_lha_amiga_212() -> io::Result<()> {
    for (name, path, size_c, size_o, crc16, crc32, modif, level, compr) in TESTS_CASES {
        println!("-------------\n{:?}", name);
        let mut lha_reader = delharc::parse_file(format!("tests/lha_amiga_212/{}", name))?;
        for filen in 0.. {
            assert!(filen <= 0);
            let mut sink = SinkSum::new();
            let header = lha_reader.header();
            assert_eq!(header.level, *level);
            let path = path.replace("*", &std::path::MAIN_SEPARATOR.to_string());
            assert_eq!(header.msdos_attrs, MsDosAttrs::HIDDEN);
            assert_eq!(header.compression_method().unwrap(), *compr);
            assert_eq!(header.compressed_size, *size_c);
            assert_eq!(header.original_size, *size_o);
            assert_eq!(&header.parse_pathname().to_str().unwrap(), &path);
            assert_eq!(&header.parse_pathname_to_str(), &path);
            let last_modified = format!("{}", header.parse_last_modified());
            assert_eq!(&last_modified, modif);
            assert_eq!(header.file_crc, *crc16);
            if header.level == 0 {
                assert_eq!(header.parse_os_type()?, OsType::Generic);
            }
            else {
                assert_eq!(header.parse_os_type()?, OsType::Amiga);
            }
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
            if !lha_reader.next_file().unwrap() {
                break;
            }
        }
    }
    Ok(())
}
