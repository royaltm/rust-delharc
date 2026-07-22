#![cfg(feature = "std")]
use std::io;
use delharc::header::*;

mod sink;
use sink::SinkSum;

use CompressionMethod::*;
const TESTS_CASES: &[(&str, &str, u64, u64, u16, u32, &str, u8, CompressionMethod)] = &[
    ("abspath.lzh", "Mounted Volume*subdir*subdir2*hello.txt",
                    12,    12, 0x9778, 0xAF083B2D, "2025-06-29 00:11:44 UTC", 2, Lh0),
    ("lh0.lzh",     "Mounted Volume*gpl-2.gz",
                  6829,  6829, 0xB6D5, 0xE4690583, "2025-06-28 23:46:06 UTC", 2, Lh0),
    ("lh5.lzh",     "Mounted Volume*gpl-2",
                  6996, 18092, 0xA33A, 0x4E46F4A1, "2025-06-28 23:46:06 UTC", 2, Lh5),
];

#[test]
fn test_tascal_lha_051h() -> io::Result<()> {
    for (name, path, size_c, size_o, crc16, crc32, modif, level, compr) in TESTS_CASES {
        println!("-------------\n{:?}", name);
        let mut lha_reader = delharc::parse_file(format!("tests/tascal_lha_051h/{}", name))?;
        let mut sink = SinkSum::new();
        let header = lha_reader.header();
        assert_eq!(header.level, *level);
        assert!(!header.is_directory());
        assert_eq!(header.parse_os_type()?, OsType::MsDos);
        assert_eq!(header.msdos_attrs, MsDosAttrs::ARCHIVE);
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
        assert!(!lha_reader.seek_next_file().unwrap());
    }
    Ok(())
}
