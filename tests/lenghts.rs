use std::{io, fs};
use delharc::{*, decode::DecoderAny};

mod sink;
use sink::SinkSum;

const TESTS_CASES: &[(&str, &str, u64, u64, u16, u32, &str, u8, CompressionMethod)] = &[
    ("lh1-0.lzh",   "0.bin",        0,       0, 0x0000, 0x00000000, "2011-07-03 18:05:35 UTC", 1, CompressionMethod::Lh1),
    ("lh1-1.lzh",   "1.BIN",        1,       1, 0x0000, 0xD202EF8D, "2011-07-03 19:00:16", 0, CompressionMethod::Lh0),
    ("lh1-1m.lzh",  "1M.BIN",   21888, 1048576, 0x0000, 0xA738EA1C, "2011-07-03 19:00:38", 0, CompressionMethod::Lh1),
    ("lh1-2m.lzh",  "2M.BIN",   43733, 2097152, 0x0000, 0x8D89877E, "2011-07-03 23:32:12", 0, CompressionMethod::Lh1),
    ("lh1-64k.lzh", "65536.BIN", 1408,   65536, 0x0000, 0xD7978EEB, "2011-07-03 19:00:28", 0, CompressionMethod::Lh1),
    ("lz5-0.lzs",   "0.BIN",        0,       0, 0x0000, 0x00000000, "2011-07-03 19:05:34", 0, CompressionMethod::Lz4),
    ("lz5-1.lzs",   "1.BIN",        1,       1, 0x0000, 0xD202EF8D, "2011-07-03 19:00:16", 0, CompressionMethod::Lz4),
    ("lz5-1m.lzs",  "1M.BIN",  123793, 1048576, 0x0000, 0xA738EA1C, "2011-07-03 19:00:38", 0, CompressionMethod::Lz5),
    ("lz5-64k.lzs", "65536.BIN", 7739,   65536, 0x0000, 0xD7978EEB, "2011-07-03 19:00:28", 0, CompressionMethod::Lz5),
];

#[test]
fn test_lengths() -> io::Result<()> {
    let mut lha_reader: delharc::LhaDecodeReader::<fs::File> = Default::default();
    assert!(!lha_reader.is_present());
    assert!(lha_reader.is_absent());
    assert!(lha_reader.take_inner().is_none());
    for (name, path, size_c, size_o, crc16, crc32, modif, level, compr) in TESTS_CASES {
        println!("-------------\n{:?}", name);
        let mut file = fs::File::open(format!("tests/lengths/{}", name))?;
        let header = LhaHeader::read(&mut file)?.unwrap();
        let decoder = DecoderAny::new_from_header(&header, file);
        lha_reader.begin_with_header_and_decoder(header, decoder);
        loop {
            let mut sink = SinkSum::new();
            let header = lha_reader.header();
            // println!("{:?}", header);
            // for extra in header.iter_extra() {
            //     println!("{:?}", extra);
            // }
            assert!(lha_reader.is_decoder_supported());
            assert!(lha_reader.is_present());
            assert!(!lha_reader.is_absent());
            assert_eq!(header.level, *level);
            assert_eq!(header.msdos_attrs, MsDosAttrs::ARCHIVE);
            assert_eq!(&header.parse_pathname().to_str().unwrap(), path);
            if *level == 0 {
                assert_eq!(header.parse_os_type()?, OsType::Generic);
            }
            else {
                assert_eq!(header.parse_os_type()?, OsType::Unix);
            }
            assert_eq!(header.compression_method().unwrap(), *compr);
            assert_eq!(header.compressed_size, *size_c);
            assert_eq!(header.original_size, *size_o);
            assert_eq!(lha_reader.len(), *size_o);
            assert_eq!(lha_reader.is_empty(), lha_reader.len() == 0);
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
