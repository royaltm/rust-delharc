#![cfg(feature = "std")]
use std::io::{Write, Read, Seek, SeekFrom};
use std::{io, fs};
use delharc::header::*;
use delharc::decode::*;

mod sink;
use sink::SinkSum;

const TESTS_CASES: &[(&str, CompressionMethod, u64, u32, Option<usize>)] = &[
    #[cfg(feature = "lz")]
    ("lzs.bin", CompressionMethod::Lzs, 0, 0x4E46F4A1, None),
    #[cfg(feature = "lz")]
    ("lz5.bin", CompressionMethod::Lz5, 1, 0x4E46F4A1, None),
    #[cfg(feature = "lh1")]
    ("lh1.bin", CompressionMethod::Lh1, 1, 0x4E46F4A1, None),
    ("lh5.bin", CompressionMethod::Lh5, 1, 0x4E46F4A1, None),
    ("lh6.bin", CompressionMethod::Lh6, 1, 0x4E46F4A1, None),
    ("lh7.bin", CompressionMethod::Lh7, 1, 0x4E46F4A1, None),
    #[cfg(feature = "pm")]
    ("pm2.bin", CompressionMethod::Pm2,42, 0x4E46F4A1, None),
    #[cfg(feature = "pm")]
    ("pm2.bin", CompressionMethod::Pm2,34, 0x8E2093A7, Some(18176)),
    #[cfg(feature = "pm")]
    ("pm2.bin", CompressionMethod::Pm2,34, 0x369D09F4, Some(18181)),
    #[cfg(feature = "pm")]
    ("pm2.bin", CompressionMethod::Pm2, 0, 0xDA9A9301, Some(18220)),
];

#[test]
fn test_decode() {
    const CRC32: u32 = 0x4E46F4A1;
    let file = fs::File::open("tests/decode/lh0.bin").unwrap();
    let size = file.metadata().unwrap().len();
    let mut decoder = PassthroughDecoder::new(file);
    test_decoder(&mut decoder, size as usize, CRC32, usize::max_value());

    for (name, compression, offset, crc32, altsize) in TESTS_CASES {
        println!("-------------\n{:?}", name);
        let mut file = fs::File::open(format!("tests/decode/{}", name)).unwrap();
        let compressed_size = file.metadata().unwrap().len();
        for limit in [usize::max_value(), 257, 128, 31, 3, 2, 1].iter().copied() {
            let mut decoder = DecoderAny::new_from_compression(*compression,
                                          file.take(compressed_size - offset));
            assert!(decoder.is_supported());
            test_decoder(&mut decoder, altsize.unwrap_or(size as usize), *crc32, limit);
            // println!("{:?}", decoder);
            file = decoder.into_inner().into_inner();
            assert_eq!(compressed_size - offset, file.stream_position().unwrap());
            file.seek(SeekFrom::Start(0)).unwrap();
        }
    }
}

fn test_decoder<R: io::Read + io::Seek, D: Decoder<R>>(
    decoder: &mut D,
    mut target_size: usize,
    crc_validate: u32,
    buf_size: usize)
{
    let mut buffer = vec![0u8;16384];
    let mut sink = SinkSum::new();
    while target_size != 0 {
        let len = buffer.len().min(target_size).min(buf_size);
        let target = &mut buffer[0..len];
        decoder.fill_buffer(target).unwrap();
        sink.write_all(target).unwrap();
        target_size -= len;
    }
    assert_eq!(sink.crc32.get_crc(), crc_validate);
}
