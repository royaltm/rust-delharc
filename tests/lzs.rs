use std::io;
use delharc::header::*;

mod sink;
use sink::SinkSum;

const TESTS_CASES: &[(&str, &str, u64, u64, u16, u32, &str)] = &[
    ("lzs.lzs",  "GPL-2",     12667,   18092, 0xa33a, 0x4E46F4A1, "2010-05-06 23:17:54"),
    ("long.lzs", "LONG.TXT", 338485, 1241658, 0x6a7c, 0x06788E85, "2011-06-09 20:19:18"),
];

#[test]
fn test_lzs() -> io::Result<()> {
    for (name, path, size_c, size_o, crc16, crc32, modif) in TESTS_CASES {
        println!("-------------\n{:?}", name);
        let mut lha_reader = delharc::parse_file(format!("tests/lzs/{}", name))?;
        loop {
            let mut sink = SinkSum::new();
            let header = lha_reader.header();
            assert_eq!(header.level, 0);
            assert_eq!(header.msdos_attrs, MsDosAttrs::ARCHIVE);
            assert_eq!(&header.parse_pathname().to_str().unwrap(), path);
            assert_eq!(OsType::Generic, header.parse_os_type()?);
            assert_eq!(CompressionMethod::Lzs, header.compression_method().unwrap());
            assert_eq!(header.compressed_size, *size_c);
            assert_eq!(header.original_size, *size_o);
            let last_modified = format!("{}", header.parse_last_modified());
            assert_eq!(&last_modified, modif);
            assert_eq!(header.file_crc, *crc16);
            io::copy(&mut lha_reader, &mut sink)?;
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
