use std::{io::{self, Seek, SeekFrom}, fs};
use delharc::header::*;

mod sink;
use sink::SinkSum;

const TESTS_CASES: &[(u64, &str, &str, u64, u64, u16, u32, &str, u8, CompressionMethod)] = &[
    (0, "lh0.lzh",    "GPL2.GZ", 6829,  6829, 0xB6D5, 0xE4690583, "2011-12-11 18:30:36", 0, CompressionMethod::Lh0),
    (0, "lh5.lzh",    "GPL2",    7004, 18092, 0xA33A, 0x4E46F4A1, "2011-12-11 18:30:36", 0, CompressionMethod::Lh5),
    #[cfg(feature = "lz")]
    (0, "lz5.lzh",    "GPL2",    8480, 18092, 0xA33A, 0x4E46F4A1, "2011-12-11 18:30:36", 0, CompressionMethod::Lz5),
    (0, "h1_lh5.lzh", "GPL2",    7004, 18092, 0xA33A, 0x4E46F4A1, "2011-12-11 18:30:36", 1, CompressionMethod::Lh5),
    #[cfg(feature = "lz")]
    (0, "h1_lz5.lzh", "GPL2",    8480, 18092, 0xA33A, 0x4E46F4A1, "2011-12-11 18:30:36", 1, CompressionMethod::Lz5),
    (0, "h2_lh5.lzh", "GPL2",    7004, 18092, 0xA33A, 0x4E46F4A1, "2012-01-11 18:30:36 UTC", 2, CompressionMethod::Lh5),
    #[cfg(feature = "lz")]
    (0, "h2_lz5.lzh", "GPL2",    8480, 18092, 0xA33A, 0x4E46F4A1, "2012-01-11 18:30:36 UTC", 2, CompressionMethod::Lz5),
    (0, "shorter.lzh", "SHORTER.TXT", 83043, 286015, 0xEA66, 0x3FDCDA96, "2011-12-11 18:31:32", 0, CompressionMethod::Lh5),
    (0, "subdir.lzh",
        "SUBDIR*SUBDIR2*HELLO.TXT", 12, 12, 0x9778, 0xAF083B2D, "2011-12-11 18:49:36", 0, CompressionMethod::Lh0),
    (0, "h1_subdir.lzh",
        "SUBDIR*SUBDIR2*HELLO.TXT", 12, 12, 0x9778, 0xAF083B2D, "2011-12-11 18:49:36", 1, CompressionMethod::Lh0),
    (0, "h2_subdir.lzh",
        "SUBDIR*SUBDIR2*HELLO.TXT", 12, 12, 0x9778, 0xAF083B2D, "2012-01-11 18:49:36 UTC", 2, CompressionMethod::Lh0),
    (0x3C, "sfx.tos", "GPL2",  7004, 18092, 0xA33A, 0x4E46F4A1, "2011-12-11 18:30:36", 0, CompressionMethod::Lh5),
];

#[test]
fn test_lharc_atari_313a() -> io::Result<()> {
    for (offset, name, path, size_c, size_o, crc16, crc32, modif, level, compr) in TESTS_CASES {
        println!("-------------\n{:?}", name);
        let mut file = fs::File::open(format!("tests/lharc_atari_313a/{}", name))?;
        file.seek(SeekFrom::Start(*offset))?;
        let mut lha_reader = delharc::LhaDecodeReader::new(file)?;
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
                assert_eq!(header.parse_os_type()?, OsType::Atari);
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
