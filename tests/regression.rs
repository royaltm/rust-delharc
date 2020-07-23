use std::{io, fs};
use delharc::header::*;

mod sink;
use sink::SinkSum;

const TESTS_CASES: &[(&str, &[(&str, u64, u64, u16, u32, &str, u8, OsType, CompressionMethod)])] = &[
    ("abspath.lzh", &[
        ("tmp*absolute_path.txt",
            46,      46, 0x6BC0, 0xBD98D221, "2012-04-05 20:21:38 UTC", 1, OsType::Unix, CompressionMethod::Lh0)
    ]),
    ("badterm.lzh", &[
        ("tmp*%1b]2;malicious%07%0a",
             0,       0, 0x0000, 0x00000000, "2012-04-05 21:10:20 UTC", 1, OsType::Unix, CompressionMethod::Lh1)
    ]),
    ("dir.lzh",     &[
        ("dir",
             0,       0, 0x0000, 0x00000000, "2012-04-06 13:08:30 UTC", 1, OsType::Unix, CompressionMethod::Lhd)
    ]),
    ("unixsep.lzh", &[
        ("SUBDIR*SUBDIR2*HELLO.TXT",
            12,      12, 0x9778, 0xAF083B2D, "2010-01-01 00:00:00", 0, OsType::Generic, CompressionMethod::Lh0)
    ]),
    ("dotdot.lzh",  &[
        ("evil1.txt",
            13,      13, 0x3AD2, 0x3E9D1D76, "2013-01-29 20:18:48 UTC", 1, OsType::Unix, CompressionMethod::Lh0),
        ("foo*evil2.txt",
            18,      18, 0x3D30, 0x4AE45690, "2013-01-29 20:20:35 UTC", 1, OsType::Unix, CompressionMethod::Lh0)
    ]),
    ("multiple.lzh", &[
        ("file1.txt",
            11,      11, 0x3245, 0xF4DB30DF, "2000-01-01 00:00:00 UTC", 1, OsType::Unix, CompressionMethod::Lh0),
        ("file2-1.txt",
            15,      15, 0x59F1, 0xAB43A3D4, "2000-01-01 00:00:00 UTC", 1, OsType::Unix, CompressionMethod::Lh0),
        ("file2-2.txt",
            15,      15, 0x39A9, 0x71211740, "2000-01-01 00:00:00 UTC", 1, OsType::Unix, CompressionMethod::Lh0),
        ("file3.txt",
            11,      11, 0x7225, 0x68626617, "2000-01-01 00:00:00 UTC", 1, OsType::Unix, CompressionMethod::Lh0),
        ("file4.txt",
            12,      12, 0xABBE, 0xCF822EF4, "2000-01-01 00:00:00 UTC", 1, OsType::Unix, CompressionMethod::Lh0)
    ]),
    ("symlink1.lzh", &[
        ("foo.txt|bar.txt",
             0,       0, 0x0000, 0x00000000, "2013-01-29 19:57:39 UTC", 1, OsType::Unix, CompressionMethod::Lhd),
        ("foo.txt",
             12,     12, 0x9778, 0xAF083B2D, "2013-01-29 19:58:08 UTC", 1, OsType::Unix, CompressionMethod::Lh0)
    ]),
    ("symlink2.lzh", &[
        ("etc|..*etc",
             0,       0, 0x0000, 0x00000000, "2013-02-03 15:05:54 UTC", 1, OsType::Unix, CompressionMethod::Lhd),
        ("etc*passwd",
             12,     12, 0x0953, 0xC003391A, "2013-02-03 15:05:17 UTC", 1, OsType::Unix, CompressionMethod::Lh0)
    ]),
    ("symlink3.lzh", &[
        ("etc|*tmp",
             0,       0, 0x0000, 0x00000000, "2013-02-03 15:08:27 UTC", 1, OsType::Unix, CompressionMethod::Lhd),
        ("etc*passwd",
             12,     12, 0x0953, 0xC003391A, "2013-02-03 15:07:43 UTC", 1, OsType::Unix, CompressionMethod::Lh0)
    ]),
    ("truncated.lzh", &[
        ("GPL-2",
           7004,  18092, 0xA33A, 0x00000000, "2010-01-01 00:00:00", 1, OsType::MsDos, CompressionMethod::Lh5)
    ])
];

#[test]
fn test_regression() -> io::Result<()> {
    for (name, headers) in TESTS_CASES {
        println!("-------------\n{:?}", name);
        let file = fs::File::open(format!("tests/regression/{}", name))?;
        let mut lha_reader = delharc::LhaDecodeReader::new(&file)?;
        for filen in 0.. {
            assert!(filen < headers.len());
            let (path, size_c, size_o, crc16, crc32, modif, level, ostype, compr) = &headers[filen];
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
            assert_eq!(header.parse_os_type()?, *ostype);
            if *compr == CompressionMethod::Lhd {
                assert!(io::copy(&mut lha_reader, &mut sink).is_err());
            }
            else if name == &"truncated.lzh" {
                let e = io::copy(&mut lha_reader, &mut sink).unwrap_err();
                assert_eq!(e.kind(), io::ErrorKind::UnexpectedEof);
            }
            else {
                io::copy(&mut lha_reader, &mut sink)?;
                assert_eq!(sink.length, *size_o as u64);
                assert_eq!(sink.crc16.get_crc(), *crc16);
                assert_eq!(sink.crc32.get_crc(), *crc32);
                assert_eq!(lha_reader.crc_check().unwrap(), *crc16);
            }
            if !lha_reader.next_file().unwrap() {
                break;
            }
        }
    }
    Ok(())
}
