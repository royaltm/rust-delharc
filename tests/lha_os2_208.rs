use std::io;
use delharc::header::*;

mod sink;
use sink::SinkSum;

const TESTS_CASES: &[(&str, &str, u64, u64, u16, u32, &str, u8, CompressionMethod)] = &[
    ("lh0.lzh",    "GPL-2.gz", 6829,  6829, 0xB6D5, 0xE4690583, "2011-12-03 16:35:22", 1, CompressionMethod::Lh0),
    ("lh1.lzh",    "GPL-2",    7208, 18092, 0xA33A, 0x4E46F4A1, "2011-12-03 16:29:06", 0, CompressionMethod::Lh1),
    ("lh5.lzh",    "GPL-2",    7004, 18092, 0xA33A, 0x4E46F4A1, "2011-12-03 16:29:06", 1, CompressionMethod::Lh5),
    ("lfn.lzh",
        "Long Filename.txt",     14,    14, 0x3197, 0xB19B306E, "2011-12-03 16:38:50", 1, CompressionMethod::Lh0),
    ("h3_lh0.lzh", "GPL-2.gz", 6829,  6829, 0xB6D5, 0xE4690583, "2011-12-03 21:35:22 UTC", 3, CompressionMethod::Lh0),
    ("h3_lh5.lzh", "GPL-2",    7004, 18092, 0xA33A, 0x4E46F4A1, "2011-12-03 21:29:06 UTC", 3, CompressionMethod::Lh5),
    ("h3_lfn.lzh",
        "Long Filename.txt",     14,    14, 0x3197, 0xB19B306E, "2011-12-03 21:38:50 UTC", 3, CompressionMethod::Lh0),
];

const SUBDIR_CASES: &[(&str, &[(&str, u64, u64, u16, u32, &str, u8, CompressionMethod)])] = &[
    ("subdir.lzh", &[
        ("subdir",                    0,  0, 0x0000, 0x00000000, "2011-12-03 16:47:56", 1, CompressionMethod::Lhd),
        ("subdir*subdir2",            0,  0, 0x0000, 0x00000000, "2011-12-03 16:47:58", 1, CompressionMethod::Lhd),
        ("subdir*subdir2*HELLO.TXT", 14, 14, 0x3197, 0xB19B306E, "2011-12-03 16:38:50", 1, CompressionMethod::Lh0)]),
    ("h3_subdir.lzh", &[
        ("subdir",                    0,  0, 0x0000, 0x00000000, "2011-12-03 21:47:56 UTC", 3, CompressionMethod::Lhd),
        ("subdir*subdir2",            0,  0, 0x0000, 0x00000000, "2011-12-03 21:47:58 UTC", 3, CompressionMethod::Lhd),
        ("subdir*subdir2*HELLO.TXT", 14, 14, 0x3197, 0xB19B306E, "2011-12-03 21:38:50 UTC", 3, CompressionMethod::Lh0)]),
];


#[test]
fn test_lha_os2_208() -> io::Result<()> {
    for (name, path, size_c, size_o, crc16, crc32, modif, level, compr) in TESTS_CASES {
        println!("-------------\n{:?}", name);
        let mut lha_reader = delharc::parse_file(format!("tests/lha_os2_208/{}", name))?;
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
            if header.level == 0 {
                assert_eq!(header.parse_os_type()?, OsType::Generic);
            }
            else {
                assert_eq!(header.parse_os_type()?, OsType::Os2);
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

    for (name, headers) in SUBDIR_CASES {
        println!("-------------\n{:?}", name);
        let mut lha_reader = delharc::parse_file(format!("tests/lha_os2_208/{}", name))?;
        for filen in 0.. {
            assert!(filen < headers.len());
            let (path, size_c, size_o, crc16, crc32, modif, level, compr) = &headers[filen];
            let mut sink = SinkSum::new();
            let header = lha_reader.header();
            assert_eq!(header.level, *level);
            let path = path.replace("*", &std::path::MAIN_SEPARATOR.to_string());
            if header.compression_method().unwrap() == CompressionMethod::Lhd {
                assert_eq!(header.msdos_attrs, MsDosAttrs::empty());
            }
            else {
                assert_eq!(header.msdos_attrs, MsDosAttrs::ARCHIVE);
            }
            assert_eq!(header.compression_method().unwrap(), *compr);
            assert_eq!(header.compressed_size, *size_c);
            assert_eq!(header.original_size, *size_o);
            assert_eq!(&header.parse_pathname().to_str().unwrap(), &path);
            let last_modified = format!("{}", header.parse_last_modified());
            assert_eq!(&last_modified, modif);
            assert_eq!(header.file_crc, *crc16);
            assert_eq!(header.parse_os_type()?, OsType::Os2);
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
