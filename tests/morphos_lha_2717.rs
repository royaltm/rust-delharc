#![cfg(feature = "std")]
use std::io;
use delharc::header::*;

mod sink;
use sink::SinkSum;

use CompressionMethod::*;
const TESTS_CASES: &[(&str, &str, u64, u64, u16, u32, &str, u8, CompressionMethod, Option<&str>)] = &[
    ("h0_lh0.lzh", "gpl-2.gz",          6829,  6829, 0xB6D5, 0xE4690583, "2010-01-01 05:00:00", 0, Lh0, None),
    #[cfg(feature = "lh1")]
    ("h0_lh1.lzh", "gpl-2",             7201, 18092, 0xA33A, 0x4E46F4A1, "2010-01-01 05:00:00", 0, Lh1, None),
    ("h0_lh5.lzh", "gpl-2",             6996, 18092, 0xA33A, 0x4E46F4A1, "2010-01-01 05:00:00", 0, Lh5, None),
    ("h0_lh6.lzh", "gpl-2",             6832, 18092, 0xA33A, 0x4E46F4A1, "2010-01-01 05:00:00", 0, Lh6, None),
    ("h0_metadata.lzh", "metadata.txt",   29,    29, 0xD1B8, 0xF0771CC4, "2025-07-03 00:33:32", 0, Lh0,
                                                                        Some("This is a comment on the file.")),
    ("h0_subdir.lzh", "subdir*subdir2*hello.txt",
                                          12,    12, 0x9778, 0xAF083B2D, "2010-01-01 05:00:00", 0, Lh0, None),
    ("h1_lh0.lzh", "gpl-2.gz",          6829,  6829, 0xB6D5, 0xE4690583, "2010-01-01 05:00:00", 1, Lh0, None),
    #[cfg(feature = "lh1")]
    ("h1_lh1.lzh", "gpl-2",             7201, 18092, 0xA33A, 0x4E46F4A1, "2010-01-01 05:00:00", 1, Lh1, None),
    ("h1_lh5.lzh", "gpl-2",             6996, 18092, 0xA33A, 0x4E46F4A1, "2010-01-01 05:00:00", 1, Lh5, None),
    ("h1_lh6.lzh", "gpl-2",             6832, 18092, 0xA33A, 0x4E46F4A1, "2010-01-01 05:00:00", 1, Lh6, None),
    ("h1_metadata.lzh", "metadata.txt",   29,    29, 0xD1B8, 0xF0771CC4, "2025-07-03 00:33:32", 1, Lh0,
                                                                        Some("This is a comment on the file.")),
    ("h1_subdir.lzh", "subdir*subdir2*hello.txt",
                                          12,    12, 0x9778, 0xAF083B2D, "2010-01-01 05:00:00", 1, Lh0, None),

    ("h2_lh0.lzh", "gpl-2.gz",          6829,  6829, 0xB6D5, 0xE4690583, "2010-01-01 05:00:00 UTC", 2, Lh0, None),
    #[cfg(feature = "lh1")]
    ("h2_lh1.lzh", "gpl-2",             7201, 18092, 0xA33A, 0x4E46F4A1, "2010-01-01 05:00:00 UTC", 2, Lh1, None),
    ("h2_lh5.lzh", "gpl-2",             6996, 18092, 0xA33A, 0x4E46F4A1, "2010-01-01 05:00:00 UTC", 2, Lh5, None),
    ("h2_lh6.lzh", "gpl-2",             6832, 18092, 0xA33A, 0x4E46F4A1, "2010-01-01 05:00:00 UTC", 2, Lh6, None),
    ("h2_metadata.lzh", "metadata.txt",   29,    29, 0xD1B8, 0xF0771CC4, "2025-07-03 00:33:32 UTC", 2, Lh0,
                                                                        Some("This is a comment on the file.")),
    ("h2_subdir.lzh", "subdir*subdir2*hello.txt",
                                          12,    12, 0x9778, 0xAF083B2D, "2010-01-01 05:00:00 UTC", 2, Lh0, None),
    ("h2_huge.lzh", "zero.bin",   23891, 4718592000, 0x0000, 0xF01352BE, "2025-07-02 18:15:04 UTC", 2, Lh5, None),
];

fn test_morphos_lha_2717_impl(allow_long_test: bool) -> io::Result<()> {
    for (name, path, size_c, size_o, crc16, crc32, modif, level, compr, comment) in TESTS_CASES {
        println!("-------------\n{:?}", name);
        let mut lha_reader = delharc::parse_file(format!("tests/morphos_lha_2717/{}", name))?;
        let mut sink = SinkSum::new();
        let header = lha_reader.header();
        assert_eq!(header.level, *level);
        assert_eq!(header.msdos_attrs, MsDosAttrs::empty());
        assert_eq!(header.compression_method().unwrap(), *compr);
        assert_eq!(header.compressed_size, *size_c);
        assert_eq!(header.original_size, *size_o);
        let path1 = path.replace("*", &std::path::MAIN_SEPARATOR.to_string());
        assert_eq!(&header.parse_pathname().to_str().unwrap(), &path1);
        let path1 = path.replace("*", "/");
        assert_eq!(&header.parse_pathname_to_str(), &path1);
        if let Some(comment) = comment {
            assert_eq!(header.parse_comment().unwrap(), *comment);
        }
        else {
            assert!(header.parse_comment().is_none());
        }
        assert!(header.parse_os_9_attrs().is_none());
        assert!(header.parse_unix_permissions().is_none());
        assert!(header.parse_unix_uid_gid().is_none());
        assert!(!header.is_directory());
        let last_modified = format!("{}", header.parse_last_modified());
        assert_eq!(&last_modified, modif);
        assert_eq!(header.file_crc, *crc16);
        if header.level == 0 {
            assert_eq!(header.parse_os_type()?, OsType::Generic);
        }
        else {
            assert_eq!(header.parse_os_type()?, OsType::Amiga);
        }
        if allow_long_test || header.original_size <= u64::from(u32::MAX) {
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
        }
        assert!(!lha_reader.seek_next_file().unwrap());
    }
    Ok(())
}

#[test]
fn test_morphos_lha_2717() -> io::Result<()> {
    test_morphos_lha_2717_impl(false)
}

#[test]
#[ignore = "long tests"]
fn test_morphos_lha_2717_long() -> io::Result<()> {
    let mut allow_long_test = false;
    if let Ok(val) = std::env::var("TEST_LONG_FILES") && val != "0" {
        allow_long_test = true;
    }
    test_morphos_lha_2717_impl(allow_long_test)
}
