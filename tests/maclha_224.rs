#![cfg(feature = "std")]
use std::io;
use delharc::header::*;

mod sink;
use sink::SinkSum;

use CompressionMethod::*;
const TESTS_CASES: &[(&str, &str, u64, u64, u16, u32, &str, u8, CompressionMethod)] = &[
    ("l0_lh0.lzh", "gpl-2.gz", 7040,   7040, 0xBEE7, 0x12789183, "2012-04-17 16:56:58", 0, Lh0),
    #[cfg(feature = "lh1")]
    ("l0_lh1.lzh", "gpl-2",    7250,  18304, 0x12DD, 0xF96B05FD, "2012-04-17 16:56:58", 0, Lh1),
    ("l0_lh5.lzh", "gpl-2",    7061,  18304, 0x12DD, 0xF96B05FD, "2012-04-17 16:56:58", 0, Lh5),
    ("l0_nm_lh5.lzh", "gpl-2", 7004,  18092, 0xA33A, 0x4E46F4A1, "2012-04-17 16:56:58", 0, Lh5),
    ("l1_full_subdir.lzh", "Untitled*subdir*subdir2*hello.txt",
                                256,    256, 0xEDA1, 0xEDBDC12C, "2012-04-19 19:10:02", 1, Lh0),
    ("l1_lh0.lzh", "gpl-2.gz", 7040,   7040, 0xBEE7, 0x12789183, "2012-04-17 16:56:58", 1, Lh0),
    #[cfg(feature = "lh1")]
    ("l1_lh1.lzh", "gpl-2",    7250,  18304, 0x12DD, 0xF96B05FD, "2012-04-17 16:56:58", 1, Lh1),
    ("l1_lh5.lzh", "gpl-2",    7061,  18304, 0x12DD, 0xF96B05FD, "2012-04-17 16:56:58", 1, Lh5),
    ("l1_nm_lh5.lzh", "gpl-2", 7004,  18092, 0xA33A, 0x4E46F4A1, "2012-04-17 16:56:58", 1, Lh5),
    ("l1_subdir.lzh", "subdir*subdir2*hello.txt",
                                256,    256, 0xEDA1, 0xEDBDC12C, "2012-04-19 19:10:02", 1, Lh0),
    ("l2_full_subdir.lzh", "Untitled*subdir*subdir2*hello.txt",
                                256,    256, 0xEDA1, 0xEDBDC12C, "2012-04-19 18:10:02 UTC", 2, Lh0),
    ("l2_lh0.lzh", "gpl-2.gz", 7040,   7040, 0xBEE7, 0x12789183, "2012-04-17 15:56:58 UTC", 2, Lh0),
    #[cfg(feature = "lh1")]
    ("l2_lh1.lzh", "gpl-2",    7250,  18304, 0x12DD, 0xF96B05FD, "2012-04-17 15:56:58 UTC", 2, Lh1),
    ("l2_lh5.lzh", "gpl-2",    7061,  18304, 0x12DD, 0xF96B05FD, "2012-04-17 15:56:58 UTC", 2, Lh5),
    ("l2_nm_lh5.lzh", "gpl-2", 7004,  18092, 0xA33A, 0x4E46F4A1, "2012-04-17 15:56:58 UTC", 2, Lh5),
    ("l2_subdir.lzh", "subdir*subdir2*hello.txt",
                                256,    256, 0xEDA1, 0xEDBDC12C, "2012-04-19 18:10:02 UTC", 2, Lh0),
];
// TODO: check MacHeader content
#[test]
fn test_maclha_224() -> io::Result<()> {
    for (name, path, size_c, size_o, crc16, crc32, modif, level, compr) in TESTS_CASES {
        println!("-------------\n{:?}", name);
        let mut lha_reader = delharc::parse_file(format!("tests/maclha_224/{}", name))?;
        let mut sink = SinkSum::new();
        let header = lha_reader.header();
        assert_eq!(header.level, *level);
        assert_eq!(header.msdos_attrs, MsDosAttrs::ARCHIVE);
        assert_eq!(header.compression_method().unwrap(), *compr);
        assert_eq!(header.compressed_size, *size_c);
        assert_eq!(header.original_size, *size_o);
        let path1 = path.replace("*", &std::path::MAIN_SEPARATOR.to_string());
        assert_eq!(&header.parse_pathname().to_str().unwrap(), &path1);
        let path1 = path.replace("*", "/");
        assert_eq!(&header.parse_pathname_to_str(), &path1);
        assert!(header.parse_comment().is_none());
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
            assert_eq!(header.parse_os_type()?, OsType::MacOs);
        }
        io::copy(&mut lha_reader, &mut sink)?;
        assert_eq!(sink.length, *size_o as u64);
        assert_eq!(sink.crc32.get_crc(), *crc32);
        assert_eq!(sink.crc16.get_crc(), *crc16);
        assert_eq!(lha_reader.crc_check().unwrap(), *crc16);
        assert!(!lha_reader.seek_next_file().unwrap());
    }
    Ok(())
}
