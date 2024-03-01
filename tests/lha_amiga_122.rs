use std::{io::{self, Seek, SeekFrom}, fs};
use delharc::header::*;

mod sink;
use sink::SinkSum;

const TESTS_CASES: &[(u64, &str, &str, u64, u64, u16, u32, &str, u8, CompressionMethod)] = &[
    (0, "lh0.lzh",    "gpl-2.gz", 6829,  6829, 0xB6D5, 0xE4690583, "1980-06-12 21:03:18", 0, CompressionMethod::Lh0),
    #[cfg(feature = "lh1")]
    (0, "lh1.lzh",    "gpl-2",    7201, 18092, 0xA33A, 0x4E46F4A1, "1980-06-12 21:03:18", 0, CompressionMethod::Lh1),
    (0, "lh4.lzh",    "gpl-2",    7095, 18092, 0xA33A, 0x4E46F4A1, "1980-06-12 21:03:18", 0, CompressionMethod::Lh4),
    (0, "lh5.lzh",    "gpl-2",    6996, 18092, 0xA33A, 0x4E46F4A1, "1980-06-12 21:03:18", 0, CompressionMethod::Lh5),
    (0, "lh4_long.lzh", "long.txt", 86725, 1241658, 0x6A7C, 0x06788E85, "1980-06-12 21:12:04", 0, CompressionMethod::Lh4),
    (0, "subdir.lzh",
        "subdir*subdir2*hello.txt", 12, 12, 0x9778, 0xAF083B2D, "1980-06-12 21:06:54", 0, CompressionMethod::Lh0),
    (0, "level0.lzh",
        "subdir*subdir2*hello.txt", 12, 12, 0x9778, 0xAF083B2D, "1980-06-12 21:06:54", 0, CompressionMethod::Lh0),
    (0, "level1.lzh",
        "subdir*subdir2*hello.txt", 12, 12, 0x9778, 0xAF083B2D, "1980-06-12 21:06:54", 1, CompressionMethod::Lh0),
    (0, "level2.lzh",
        "subdir*subdir2*hello.txt", 12, 12, 0x9778, 0xAF083B2D, "1980-06-12 21:06:54 UTC", 2, CompressionMethod::Lh0),
    (0x15DC, "sfx.run", "SFXUsage.txt",  478, 832, 0x4CAF, 0x817106CA, "1991-12-10 21:50:44", 0, CompressionMethod::Lh5),
    (0x17E0, "sfx.run", "gpl-2",  7095, 18092, 0xA33A, 0x4E46F4A1, "1980-06-12 21:03:18", 0, CompressionMethod::Lh4),
];

#[test]
fn test_lha_amiga_122() -> io::Result<()> {
    for (offset, name, path, size_c, size_o, crc16, crc32, modif, level, compr) in TESTS_CASES {
        println!("-------------\n{:?}", name);
        let mut file = fs::File::open(format!("tests/lha_amiga_122/{}", name))?;
        file.seek(SeekFrom::Start(*offset))?;
        let mut lha_reader = delharc::LhaDecodeReader::new(file)?;
        for filen in 0.. {
            assert!(filen <= 0);
            let mut sink = SinkSum::new();
            let header = lha_reader.header();
            assert_eq!(header.level, *level);
            let path = path.replace("*", &std::path::MAIN_SEPARATOR.to_string());
            if path == "SFXUsage.txt" {
                assert_eq!(header.msdos_attrs, MsDosAttrs::empty());
            }
            else {
                assert_eq!(header.msdos_attrs, MsDosAttrs::HIDDEN);
            }
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
