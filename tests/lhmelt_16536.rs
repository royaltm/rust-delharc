use std::path::PathBuf;
use std::{io::{self, Seek, SeekFrom}, fs};
use delharc::header::*;

mod sink;
use sink::SinkSum;

const TESTS_CASES: &[(u64, &str, &str, u64, u64, u16, u32, &str, u8, CompressionMethod)] = &[
    (0x3A7A, "sfx_winsfx_213.exe",    "gpl-2", 6992, 18092, 0xA33A, 0x4E46F4A1, "2010-01-01 01:00:00", 0, CompressionMethod::Lh5),
    (0x416A, "sfx_winsfxm_250.exe",   "gpl-2", 6992, 18092, 0xA33A, 0x4E46F4A1, "2010-01-01 01:00:00", 0, CompressionMethod::Lh5),
    (0x7C00, "sfx_winsfx32_213.exe",  "gpl-2", 6992, 18092, 0xA33A, 0x4E46F4A1, "2010-01-01 00:00:00 UTC", 2, CompressionMethod::Lh5),
    (0xD000, "sfx_winsfx32m_250.exe", "gpl-2", 6992, 18092, 0xA33A, 0x4E46F4A1, "2010-01-01 00:00:00 UTC", 2, CompressionMethod::Lh5),
    (0, "h0_lh0.lzh", "gpl-2.gz", 6829,  6829, 0xB6D5, 0xE4690583, "2010-01-01 01:00:00", 0, CompressionMethod::Lh0),
    #[cfg(feature = "lh1")]
    (0, "h0_lh1.lzh", "gpl-2",    7199, 18092, 0xA33A, 0x4E46F4A1, "2010-01-01 01:00:00", 0, CompressionMethod::Lh1),
    (0, "h0_lh5.lzh", "gpl-2",    6992, 18092, 0xA33A, 0x4E46F4A1, "2010-01-01 01:00:00", 0, CompressionMethod::Lh5),
    (0, "h0_lh6.lzh", "gpl-2",    6828, 18092, 0xA33A, 0x4E46F4A1, "2010-01-01 01:00:00", 0, CompressionMethod::Lh6),
    (0, "h0_lh7.lzh", "gpl-2",    6828, 18092, 0xA33A, 0x4E46F4A1, "2010-01-01 01:00:00", 0, CompressionMethod::Lh7),
    (0, "h1_lh0.lzh", "gpl-2.gz", 6829,  6829, 0xB6D5, 0xE4690583, "2010-01-01 01:00:00", 1, CompressionMethod::Lh0),
    #[cfg(feature = "lh1")]
    (0, "h1_lh1.lzh", "gpl-2",    7199, 18092, 0xA33A, 0x4E46F4A1, "2010-01-01 01:00:00", 1, CompressionMethod::Lh1),
    (0, "h1_lh5.lzh", "gpl-2",    6992, 18092, 0xA33A, 0x4E46F4A1, "2010-01-01 01:00:00", 1, CompressionMethod::Lh5),
    (0, "h2_lh0.lzh", "gpl-2.gz", 6829,  6829, 0xB6D5, 0xE4690583, "2010-01-01 00:00:00 UTC", 2, CompressionMethod::Lh0),
    #[cfg(feature = "lh1")]
    (0, "h2_lh1.lzh", "gpl-2",    7199, 18092, 0xA33A, 0x4E46F4A1, "2010-01-01 00:00:00 UTC", 2, CompressionMethod::Lh1),
    (0, "h2_lh5.lzh", "gpl-2",    6992, 18092, 0xA33A, 0x4E46F4A1, "2010-01-01 00:00:00 UTC", 2, CompressionMethod::Lh5),
    (0, "h0_subdir.lzh",
        "subdir*subdir2*hello.txt", 12, 12, 0x9778, 0xAF083B2D, "2010-01-01 01:00:00", 0, CompressionMethod::Lh0),
    (0, "h1_subdir.lzh",
        "subdir*subdir2*hello.txt", 12, 12, 0x9778, 0xAF083B2D, "2010-01-01 01:00:00", 1, CompressionMethod::Lh0),
    (0, "h2_subdir.lzh",
        "subdir*subdir2*hello.txt", 12, 12, 0x9778, 0xAF083B2D, "2010-01-01 00:00:00 UTC", 2, CompressionMethod::Lh0),
];

#[test]
fn test_lhmelt_16536() -> io::Result<()> {
    let mut lha_reader: delharc::LhaDecodeReader::<fs::File> = Default::default();
    for (offset, name, path, size_c, size_o, crc16, crc32, modif, level, compr) in TESTS_CASES {
        println!("-------------\n{:?}", name);
        let mut file = fs::File::open(format!("tests/lhmelt_16536/{}", name))?;
        file.seek(SeekFrom::Start(*offset))?;
        assert!(lha_reader.begin_new(file)?);
        for filen in 0.. {
            assert!(filen <= 1);
            let mut sink = SinkSum::new();
            let header = lha_reader.header();
            assert_eq!(header.level, *level);
            let path = path.replace("*", &std::path::MAIN_SEPARATOR.to_string());
            if filen == 1 {
                assert_eq!(header.msdos_attrs, MsDosAttrs::SUBDIR);
                assert_eq!(header.compression_method().unwrap(), CompressionMethod::Lhd);
                assert_eq!(header.compressed_size, 0);
                assert_eq!(header.original_size, 0);
                let mut fullpath = PathBuf::from(path);
                fullpath.pop();
                assert_eq!(&header.parse_pathname().to_str().unwrap(), &fullpath.to_str().unwrap());
                let last_modified = format!("{}", header.parse_last_modified());
                if header.level == 2 {
                    assert_eq!(&last_modified, "2000-01-01 00:00:00 UTC");
                }
                else {
                    assert_eq!(&last_modified, "2000-01-01 01:00:00");
                }
                assert_eq!(header.file_crc, 0);
            }
            else {
                assert_eq!(header.msdos_attrs, MsDosAttrs::ARCHIVE);
                assert_eq!(header.compression_method().unwrap(), *compr);
                assert_eq!(header.compressed_size, *size_c);
                assert_eq!(header.original_size, *size_o);
                assert_eq!(&header.parse_pathname().to_str().unwrap(), &path);
                let last_modified = format!("{}", header.parse_last_modified());
                assert_eq!(&last_modified, modif);
                assert_eq!(header.file_crc, *crc16);
            }
            if *level == 0 {
                assert_eq!(header.parse_os_type()?, OsType::Generic);
            }
            else {
                assert_eq!(header.parse_os_type()?, OsType::MsDos);
            }
            if filen == 1 {
                assert!(io::copy(&mut lha_reader, &mut sink).is_err());
                assert_eq!(sink.length, 0);
                assert_eq!(sink.crc32.get_crc(), 0);
                assert_eq!(sink.crc16.get_crc(), 0);
                assert_eq!(lha_reader.crc_check().unwrap(), 0);
            }
            else {
                io::copy(&mut lha_reader, &mut sink)?;
                assert_eq!(sink.length, *size_o as u64);
                assert_eq!(sink.crc32.get_crc(), *crc32);
                assert_eq!(sink.crc16.get_crc(), *crc16);
                assert_eq!(lha_reader.crc_check().unwrap(), *crc16);
            }
            if !lha_reader.next_file().unwrap() {
                break;
            }
        }
    }
    Ok(())
}
