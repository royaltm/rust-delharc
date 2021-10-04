use std::io;
use delharc::header::*;

mod sink;
use sink::SinkSum;

const TESTS_CASES: &[(&str, &str, u64, u64, u16, u32, &str, u8, CompressionMethod)] = &[
    ("h0_lh0.lzh", "gpl2.gz", 6829,  6829, 0xB6D5, 0xE4690583, "2012-09-22 15:57:00", 0, CompressionMethod::Lh0),
    #[cfg(feature = "lh1")]
    ("h0_lh1.lzh", "gpl2",    7514, 18092, 0xA33A, 0x4E46F4A1, "1980-01-01 00:00:02", 0, CompressionMethod::Lh1),
    ("h0_subdir.lzh",
        "SUBDIR*SUBDIR2*hello.txt", 12, 12, 0x9778, 0xAF083B2D, "2012-09-22 15:59:00", 0, CompressionMethod::Lh0),
    ("h1_lh0.lzh", "gpl2.gz", 6829,  6829, 0xB6D5, 0xE4690583, "2012-09-22 15:57:00", 1, CompressionMethod::Lh0),
    #[cfg(feature = "lh1")]
    ("h1_lh1.lzh", "gpl2",    7514, 18092, 0xA33A, 0x4E46F4A1, "1980-01-01 00:00:02", 1, CompressionMethod::Lh1),
    ("h1_subdir.lzh",
        "SUBDIR*SUBDIR2*hello.txt", 12, 12, 0x9778, 0xAF083B2D, "2012-09-22 15:59:00", 1, CompressionMethod::Lh0),
    ("h2_lh0.lzh", "gpl2.gz", 6829,  6829, 0xB6D5, 0xE4690583, "2012-09-22 20:57:01 UTC", 2, CompressionMethod::Lh0),
    #[cfg(feature = "lh1")]
    ("h2_lh1.lzh", "gpl2",    7514, 18092, 0xA33A, 0x4E46F4A1, "1970-01-01 00:00:58 UTC", 2, CompressionMethod::Lh1),
    ("h2_subdir.lzh",
        "SUBDIR*SUBDIR2*hello.txt", 12, 12, 0x9778, 0xAF083B2D, "2012-09-22 20:59:01 UTC", 2, CompressionMethod::Lh0),
];

#[test]
fn test_lha_os9_211c() -> io::Result<()> {
    for (name, path, size_c, size_o, crc16, crc32, modif, level, compr) in TESTS_CASES {
        println!("-------------\n{:?}", name);
        let mut lha_reader = delharc::parse_file(format!("tests/lha_os9_211c/{}", name))?;
        for filen in 0.. {
            assert!(filen <= 0);
            let mut sink = SinkSum::new();
            let header = lha_reader.header();
            assert_eq!(header.level, *level);
            let path = path.replace("*", &std::path::MAIN_SEPARATOR.to_string());
            assert_eq!(header.msdos_attrs, MsDosAttrs::ARCHIVE);
            assert_eq!(header.compression_method().unwrap(), *compr);
            assert_eq!(header.compressed_size, *size_c);
            assert_eq!(header.original_size, *size_o);
            assert_eq!(&header.parse_pathname().to_str().unwrap(), &path);
            let last_modified = format!("{}", header.parse_last_modified());
            assert_eq!(&last_modified, modif);
            assert_eq!(header.file_crc, *crc16);
            assert_eq!(header.parse_os_type()?, OsType::Os9);
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
