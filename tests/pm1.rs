#![cfg(all(feature = "std", feature = "pm"))]
use std::io;
use delharc::header::*;

mod sink;
use sink::SinkSum;

const PM1_FILE: &str = "pm1.pma";

const TESTS_CASES: &[(&str, u64, u64, u16, u32, &str)] = &[
    ("DATA_00.BIN", 24576, 32768, 0x2d48, 0x41FE9947, "-"),
    ("DATA_01.BIN", 24576, 32768, 0x9395, 0x2A0171D8, "-"),
    ("DATA_02.BIN", 24576, 32768, 0xf8fd, 0x421FA23D, "-"),
    ("DATA_03.BIN", 24576, 32768, 0xfaa8, 0x7AC1A551, "-"),
    ("DATA_04.BIN", 24576, 32768, 0x5424, 0x86663721, "-"),
    ("DATA_05.BIN", 24576, 32768, 0xaee7, 0x305764D8, "-"),
    ("DATA_06.BIN", 24576, 32768, 0x892f, 0x8414CBA1, "-"),
    ("DATA_07.BIN", 24576, 32768, 0x2d2c, 0x54CEA163, "-"),
    ("DATA_08.BIN", 24576, 32768, 0xfeda, 0x7DF0239C, "-"),
    ("DATA_09.BIN", 24576, 32768, 0xbb58, 0x4AF6DDAD, "-"),
    ("DATA_10.BIN", 24576, 32768, 0xd839, 0x9AF6C55D, "-"),
    ("DATA_11.BIN", 24576, 32768, 0xddfb, 0xEB3E9EE4, "-"),
    ("DATA_12.BIN", 24576, 32768, 0x3112, 0x31DBC00B, "-"),
    ("DATA_13.BIN", 24576, 32768, 0x9698, 0x584BB148, "-"),
    ("DATA_14.BIN", 24576, 32768, 0x97c9, 0x3AB08426, "-"),
    ("DATA_15.BIN", 24576, 32768, 0x1670, 0x9172E02D, "-"),
    ("DATA_16.BIN", 24576, 32768, 0x8e6a, 0xBC8F6053, "-"),
    ("DATA_17.BIN", 24576, 32768, 0x8bbb, 0x8B7FF7D7, "-"),
    ("DATA_18.BIN", 24576, 32768, 0x9383, 0xE0F514B2, "-"),
    ("DATA_19.BIN", 24576, 32768, 0x016d, 0x5B2AD1FA, "-"),
    ("DATA_20.BIN", 24576, 32768, 0x3118, 0x2A111E04, "-"),
    ("DATA_21.BIN", 24576, 32768, 0xa5a0, 0xBC085DC0, "-"),
    ("DATA_22.BIN", 24576, 32768, 0x022f, 0xE7665540, "-"),
    ("DATA_23.BIN", 24576, 32768, 0x232a, 0x8DC3D87A, "-"),
    ("DATA_24.BIN", 24576, 32768, 0x7607, 0xF822A965, "-"),
    ("DATA_25.BIN", 24576, 32768, 0xe4d4, 0xE6D69926, "-"),
    ("DATA_26.BIN", 24576, 32768, 0x097a, 0x28E13614, "-"),
    ("DATA_27.BIN", 24576, 32768, 0x0f2e, 0x565897B9, "-"),
    ("DATA_28.BIN", 24576, 32768, 0xfe2f, 0xC93B483F, "-"),
    ("DATA_29.BIN", 24576, 32768, 0x2c0e, 0x26E67D02, "-"),
    ("DATA_30.BIN", 24576, 32768, 0x7b07, 0xFAB5BAED, "-"),
    ("DATA_31.BIN", 24576, 32768, 0xa389, 0x1B1606D2, "-"),
];

#[test]
fn test_pm1() -> io::Result<()> {
    let name = PM1_FILE;
    let mut lha_reader = delharc::parse_file(format!("tests/pm1/{}", name))?;
    for (i, (path, size_c, size_o, crc16, crc32, modif)) in TESTS_CASES.iter().enumerate() {
        println!("-------------\n{:?} #{:02} {:?}", name, i + 1, path);
        let mut sink = SinkSum::new();
        let header = lha_reader.header();
        assert_eq!(header.level, 0);
        assert_eq!(header.msdos_attrs, MsDosAttrs::ARCHIVE);
        assert_eq!(&header.parse_pathname().to_str().unwrap(), path);
        assert_eq!(&header.parse_pathname_to_str(), path);
        assert_eq!(OsType::Generic, header.parse_os_type()?);
        assert_eq!(CompressionMethod::Pm1, header.compression_method().unwrap());
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
        assert_eq!(lha_reader.next_file().unwrap(), i < TESTS_CASES.len() - 1);
    }
    Ok(())
}
