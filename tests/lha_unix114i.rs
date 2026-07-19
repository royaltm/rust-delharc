#![cfg(feature = "std")]
use std::io;
use delharc::header::*;

mod sink;
use sink::SinkSum;

use CompressionMethod::*;

const TESTS_CASES: &[(&str, &str, u64, u64, u16, u32, &str, u8, CompressionMethod, &str)] = &[
    ("h0_lh0.lzh", "gpl-2.gz", 6829,  6829, 0xB6D5, 0xE4690583, "2010-01-01 00:00:00 UTC", 0, Lh0, "-rw-------"),
    ("h0_lh5.lzh", "gpl-2",    6996, 18092, 0xA33A, 0x4E46F4A1, "2010-01-01 00:00:00 UTC", 0, Lh5, "-r--r--r--"),
    ("h0_lh6.lzh", "gpl-2",    6832, 18092, 0xA33A, 0x4E46F4A1, "2010-01-01 00:00:00 UTC", 0, Lh6, "-r--r--r--"),
    ("h0_lh7.lzh", "gpl-2",    6832, 18092, 0xA33A, 0x4E46F4A1, "2010-01-01 00:00:00 UTC", 0, Lh7, "-r--r--r--"),
    ("h1_lh0.lzh", "gpl-2.gz", 6829,  6829, 0xB6D5, 0xE4690583, "2010-01-01 00:00:00 UTC", 1, Lh0, "-rw-------"),
    ("h1_lh5.lzh", "gpl-2",    6996, 18092, 0xA33A, 0x4E46F4A1, "2010-01-01 00:00:00 UTC", 1, Lh5, "-r--r--r--"),
    ("h1_lh6.lzh", "gpl-2",    6832, 18092, 0xA33A, 0x4E46F4A1, "2010-01-01 00:00:00 UTC", 1, Lh6, "-r--r--r--"),
    ("h1_lh7.lzh", "gpl-2",    6832, 18092, 0xA33A, 0x4E46F4A1, "2010-01-01 00:00:00 UTC", 1, Lh7, "-r--r--r--"),
    ("h2_lh0.lzh", "gpl-2.gz", 6829,  6829, 0xB6D5, 0xE4690583, "2010-01-01 00:00:00 UTC", 2, Lh0, "-rw-------"),
    ("h2_lh5.lzh", "gpl-2",    6996, 18092, 0xA33A, 0x4E46F4A1, "2010-01-01 00:00:00 UTC", 2, Lh5, "-r--r--r--"),
    ("h2_lh6.lzh", "gpl-2",    6832, 18092, 0xA33A, 0x4E46F4A1, "2010-01-01 00:00:00 UTC", 2, Lh6, "-r--r--r--"),
    ("h2_lh7.lzh", "gpl-2",    6832, 18092, 0xA33A, 0x4E46F4A1, "2010-01-01 00:00:00 UTC", 2, Lh7, "-r--r--r--"),
    ("lh6_long.lzh", "long.txt", 78932, 1241658, 0x6A7C, 0x06788E85, "2011-06-09 19:19:18 UTC", 1, Lh6, "-rw-r--r--"),
    ("lh7_long.lzh", "long.txt", 76620, 1241658, 0x6A7C, 0x06788E85, "2011-06-09 19:19:18 UTC", 1, Lh7, "-rw-r--r--"),
    ("h0_symlink.lzh", "symlink|target", 0, 0, 0x0000, 0x00000000, "2010-01-01 00:00:00 UTC", 0, Lhd, "lrwxrwxrwx"),
    ("h1_symlink.lzh", "symlink|target", 0, 0, 0x0000, 0x00000000, "2010-01-01 00:00:00 UTC", 1, Lhd, "lrwxrwxrwx"),
    ("h1_symlink2.lzh",
        "symlink|path*to*target",        0, 0, 0x0000, 0x00000000, "2013-02-03 22:11:49 UTC", 1, Lhd, "lrwxrwxrwx"),
    ("h1_symlink3.lzh",
        "subdir*symlink|*absolute*path", 0, 0, 0x0000, 0x00000000, "2013-02-03 22:12:20 UTC", 1, Lhd, "lrwxrwxrwx"),
    ("h2_symlink.lzh", "symlink|target", 0, 0, 0x0000, 0x00000000, "2010-01-01 00:00:00 UTC", 2, Lhd, "lrwxrwxrwx"),
    ("h2_symlink2.lzh",
        "symlink|path*to*target",        0, 0, 0x0000, 0x00000000, "2013-02-03 22:11:49 UTC", 2, Lhd, "lrwxrwxrwx"),
    ("h2_symlink3.lzh",
        "subdir*symlink|*absolute*path", 0, 0, 0x0000, 0x00000000, "2013-02-03 22:12:20 UTC", 2, Lhd, "lrwxrwxrwx"),
];

const SUBDIR_CASES: &[(&str, &[(&str, u64, u64, u16, u32, &str, u8, CompressionMethod, &str)])] = &[
    ("h0_subdir.lzh", &[
        (         "",  0,  0, 0x0000, 0x00000000, "2012-04-24 19:31:19 UTC", 0, Lhd, "drwx------"),
        (         "",  0,  0, 0x0000, 0x00000000, "2012-04-24 19:31:19 UTC", 0, Lhd, "dr-xr-xr-x"),
        ("hello.txt", 12, 12, 0x9778, 0xAF083B2D, "2010-01-01 00:00:00 UTC", 0, Lh0, "-rw-r--r--")]),
    ("h1_subdir.lzh", &[
        ("subdir*",                   0,  0, 0x0000, 0x00000000, "2012-04-24 19:31:19 UTC", 1, Lhd, "drwx------"),
        ("subdir*subdir2*",           0,  0, 0x0000, 0x00000000, "2012-04-24 19:31:19 UTC", 1, Lhd, "dr-xr-xr-x"),
        ("subdir*subdir2*hello.txt", 12, 12, 0x9778, 0xAF083B2D, "2010-01-01 00:00:00 UTC", 1, Lh0, "-rw-r--r--")]),
    ("h2_subdir.lzh", &[
        ("subdir*",                   0,  0, 0x0000, 0x00000000, "2012-04-24 19:31:19 UTC", 2, Lhd, "drwx------"),
        ("subdir*subdir2*",           0,  0, 0x0000, 0x00000000, "2012-04-24 19:31:19 UTC", 2, Lhd, "dr-xr-xr-x"),
        ("subdir*subdir2*hello.txt", 12, 12, 0x9778, 0xAF083B2D, "2010-01-01 00:00:00 UTC", 2, Lh0, "-rw-r--r--")]),
];

#[test]
fn test_lha_unix114i() -> io::Result<()> {
    for (name, path, size_c, size_o, crc16, crc32, modif, level, compr, perm) in TESTS_CASES {
        println!("-------------\n{:?}", name);
        let mut lha_reader = delharc::parse_file(format!("tests/lha_unix114i/{}", name))?;
        for filen in 0.. {
            assert!(filen <= 0);
            let mut sink = SinkSum::new();
            let header = lha_reader.header();

            // println!("{:?}", header.parse_pathname());
            // println!("{:?}", header.parse_pathname_to_str());
            // println!("{:?}", header);
            // for extra in header.iter_extra() {
            //     println!("{:02x?}", extra);
            // }

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
            assert_eq!(header.parse_os_type()?, OsType::Unix);
            assert!(header.parse_os_9_attrs().is_none());
            assert_eq!(header.parse_unix_permissions().unwrap().to_string(), *perm);
            assert_eq!(header.parse_unix_uid_gid().unwrap(), (1000, 1000));
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
        let mut lha_reader = delharc::parse_file(format!("tests/lha_unix114i/{}", name))?;
        for filen in 0.. {
            assert!(filen < headers.len());
            let (path, size_c, size_o, crc16, crc32, modif, level, compr, perm) = &headers[filen];
            let mut sink = SinkSum::new();
            let header = lha_reader.header();

            // println!("{:?}", header);
            // println!("{:?}", header.parse_pathname());
            // println!("{:?}", header.parse_pathname_to_str());
            // for extra in header.iter_extra() {
            //     println!("{:?}", extra);
            // }

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
            assert_eq!(header.parse_os_type()?, OsType::Unix);
            assert!(header.parse_os_9_attrs().is_none());
            assert_eq!(header.parse_unix_permissions().unwrap().to_string(), *perm);
            assert_eq!(header.parse_unix_uid_gid().unwrap(), (1000, 1000));
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
