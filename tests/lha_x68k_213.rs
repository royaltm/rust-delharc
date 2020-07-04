use std::{io::{self, Seek, SeekFrom}, fs};
use delharc::header::*;

mod sink;
use sink::SinkSum;

const TESTS_CASES: &[(u64, &str, &str, u64, u64, u16, u32, &str, u8, CompressionMethod)] = &[
    (0, "h0_lh0.lzh", "GPL-2.GZ", 6829,  6829, 0xB6D5, 0xE4690583, "2012-04-04 21:44:26", 0, CompressionMethod::Lh0),
    (0, "h0_lh5.lzh", "GPL-2",    7004, 18092, 0xA33A, 0x4E46F4A1, "2012-04-04 21:44:26", 0, CompressionMethod::Lh5),
    (0, "h0_subdir.lzh",
        "subdir*subdir2*HELLO.TXT", 12,    12, 0x9778, 0xAF083B2D, "2012-04-04 21:44:30", 0, CompressionMethod::Lh0),
    (0, "h1_lh0.lzh", "GPL-2.GZ", 6829,  6829, 0xB6D5, 0xE4690583, "2012-04-04 21:44:26", 1, CompressionMethod::Lh0),
    (0, "h1_lh5.lzh", "GPL-2",    7004, 18092, 0xA33A, 0x4E46F4A1, "2012-04-04 21:44:26", 1, CompressionMethod::Lh5),
    (0, "h2_lh0.lzh", "GPL-2.GZ", 6829,  6829, 0xB6D5, 0xE4690583, "2012-04-04 12:44:26 UTC", 2, CompressionMethod::Lh0),
    (0, "h2_lh5.lzh", "GPL-2",    7004, 18092, 0xA33A, 0x4E46F4A1, "2012-04-04 12:44:26 UTC", 2, CompressionMethod::Lh5),
    (0xD5E,  "sfx.x", "GPL-2",    7004, 18092, 0xA33A, 0x4E46F4A1, "2012-04-04 21:44:26", 0, CompressionMethod::Lh5),
];

const SUBDIR_CASES: &[(&str, &[(&str, u64, u64, u16, u32, &str, u8, CompressionMethod)])] = &[
    ("h1_subdir.lzh", &[
        ("subdir",                    0,  0, 0x0000, 0x00000000, "2012-04-05 13:26:18", 1, CompressionMethod::Lhd),
        ("subdir*subdir2",            0,  0, 0x0000, 0x00000000, "2012-04-05 13:26:22", 1, CompressionMethod::Lhd),
        ("subdir*subdir2*HELLO.TXT", 12, 12, 0x9778, 0xAF083B2D, "2012-04-04 21:44:30", 1, CompressionMethod::Lh0)]),
    ("h2_subdir.lzh", &[
        ("subdir",                    0,  0, 0x0000, 0x00000000, "2012-04-05 04:26:18 UTC", 2, CompressionMethod::Lhd),
        ("subdir*subdir2",            0,  0, 0x0000, 0x00000000, "2012-04-05 04:26:22 UTC", 2, CompressionMethod::Lhd),
        ("subdir*subdir2*HELLO.TXT", 12, 12, 0x9778, 0xAF083B2D, "2012-04-04 12:44:30 UTC", 2, CompressionMethod::Lh0)]),
];

#[test]
fn test_lha_x68k_213() -> io::Result<()> {
    for (offset, name, path, size_c, size_o, crc16, crc32, modif, level, compr) in TESTS_CASES {
        println!("-------------\n{:?}", name);
        let mut file = fs::File::open(format!("tests/lha_x68k_213/{}", name))?;
        file.seek(SeekFrom::Start(*offset))?;
        let mut lha_reader = delharc::LhaDecodeReader::new(file)?;
        for filen in 0.. {
            assert!(filen <= 0);
            let mut sink = SinkSum::new();
            let header = lha_reader.header();
            assert_eq!(header.level, *level);
            let path = path.replace("*", &std::path::MAIN_SEPARATOR.to_string());
            if header.level == 0 && *compr == CompressionMethod::Lhd {
                assert_eq!(header.msdos_attrs, MsDosAttrs::SUBDIR);
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
            if header.level == 0 {
                assert_eq!(header.parse_os_type()?, OsType::Generic);
            }
            else {
                assert_eq!(header.parse_os_type()?, OsType::Human68k);
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
        let mut lha_reader = delharc::parse_file(format!("tests/lha_x68k_213/{}", name))?;
        for filen in 0.. {
            assert!(filen < headers.len());
            let (path, size_c, size_o, crc16, crc32, modif, level, compr) = &headers[filen];
            let mut sink = SinkSum::new();
            let header = lha_reader.header();
            assert_eq!(header.level, *level);
            let path = path.replace("*", &std::path::MAIN_SEPARATOR.to_string());
            if header.compression_method().unwrap() == CompressionMethod::Lhd {
                assert_eq!(header.msdos_attrs, MsDosAttrs::SUBDIR);
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
            assert_eq!(header.parse_os_type()?, OsType::Human68k);
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
