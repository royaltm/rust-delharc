use std::io;
use delharc::header::*;

mod sink;
use sink::SinkSum;

const TESTS_CASES: &[(&str, &str, u64, u64, u16, u32, &str, u8, CompressionMethod)] = &[
    ("h0_lh0.lzh", "gpl-2.gz", 6829,  6829, 0xB6D5, 0xE4690583, "2010-01-01 06:00:00 UTC", 0, CompressionMethod::Lh0),
    #[cfg(feature = "lh1")]
    ("h0_lh1.lzh", "gpl-2",    7208, 18092, 0xA33A, 0x4E46F4A1, "2010-01-01 06:00:00 UTC", 0, CompressionMethod::Lh1),
    ("h0_lh5.lzh", "gpl-2",    7004, 18092, 0xA33A, 0x4E46F4A1, "2010-01-01 06:00:00 UTC", 0, CompressionMethod::Lh5),
    ("h1_lh0.lzh", "gpl-2.gz", 6829,  6829, 0xB6D5, 0xE4690583, "2010-01-01 06:00:00 UTC", 1, CompressionMethod::Lh0),
    #[cfg(feature = "lh1")]
    ("h1_lh1.lzh", "gpl-2",    7208, 18092, 0xA33A, 0x4E46F4A1, "2010-01-01 06:00:00 UTC", 1, CompressionMethod::Lh1),
    ("h1_lh5.lzh", "gpl-2",    7004, 18092, 0xA33A, 0x4E46F4A1, "2010-01-01 06:00:00 UTC", 1, CompressionMethod::Lh5),
    ("h2_lh0.lzh", "gpl-2.gz", 6829,  6829, 0xB6D5, 0xE4690583, "2010-01-01 06:00:00 UTC", 2, CompressionMethod::Lh0),
    #[cfg(feature = "lh1")]
    ("h2_lh1.lzh", "gpl-2",    7208, 18092, 0xA33A, 0x4E46F4A1, "2010-01-01 06:00:00 UTC", 2, CompressionMethod::Lh1),
    ("h2_lh5.lzh", "gpl-2",    7004, 18092, 0xA33A, 0x4E46F4A1, "2010-01-01 06:00:00 UTC", 2, CompressionMethod::Lh5),
];

const SUBDIR_CASES: &[(&str, &[(&str, u64, u64, u16, u32, &str, u8, CompressionMethod)])] = &[
    ("h0_subdir.lzh", &[
        ("",                          0,  0, 0x0000, 0x00000000, "2010-07-01 05:00:00 UTC", 0, CompressionMethod::Lhd),
        ("",                          0,  0, 0x0000, 0x00000000, "2010-07-01 05:00:00 UTC", 0, CompressionMethod::Lhd),
        ("hello.txt",                12, 12, 0x9778, 0xAF083B2D, "2010-01-01 06:00:00 UTC", 0, CompressionMethod::Lh0)]),
    ("h1_subdir.lzh", &[
        ("subdir",                    0,  0, 0x0000, 0x00000000, "2010-07-01 05:00:00 UTC", 1, CompressionMethod::Lhd),
        ("subdir*subdir2",            0,  0, 0x0000, 0x00000000, "2010-07-01 05:00:00 UTC", 1, CompressionMethod::Lhd),
        ("subdir*subdir2*hello.txt", 12, 12, 0x9778, 0xAF083B2D, "2010-01-01 06:00:00 UTC", 1, CompressionMethod::Lh0)]),
    ("h2_subdir.lzh", &[
        ("subdir",                    0,  0, 0x0000, 0x00000000, "2010-07-01 05:00:00 UTC", 2, CompressionMethod::Lhd),
        ("subdir*subdir2",            0,  0, 0x0000, 0x00000000, "2010-07-01 05:00:00 UTC", 2, CompressionMethod::Lhd),
        ("subdir*subdir2*hello.txt", 12, 12, 0x9778, 0xAF083B2D, "2010-01-01 06:00:00 UTC", 2, CompressionMethod::Lh0)]),
];

#[test]
fn test_lha_osk_201() -> io::Result<()> {
    for (name, path, size_c, size_o, crc16, crc32, modif, level, compr) in TESTS_CASES {
        println!("-------------\n{:?}", name);
        let mut lha_reader = delharc::parse_file(format!("tests/lha_osk_201/{}", name))?;
        for filen in 0.. {
            assert!(filen <= 0);
            let mut sink = SinkSum::new();
            let header = lha_reader.header();
            assert_eq!(header.level, *level);
            if header.level == 0 && *compr == CompressionMethod::Lhd {
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
            assert_eq!(header.parse_os_type()?, OsType::Osk);
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

    for (name, headers) in SUBDIR_CASES {
        println!("-------------\n{:?}", name);
        let mut lha_reader = delharc::parse_file(format!("tests/lha_osk_201/{}", name))?;
        for filen in 0.. {
            assert!(filen < headers.len());
            let (path, size_c, size_o, crc16, crc32, modif, level, compr) = &headers[filen];
            let mut sink = SinkSum::new();
            let header = lha_reader.header();
            assert_eq!(header.level, *level);
            if header.level == 0 && header.compression_method().unwrap() == CompressionMethod::Lhd {
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
            assert_eq!(header.parse_os_type()?, OsType::Osk);
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
