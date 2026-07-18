#[cfg(feature = "std")]
use std::io::Write;
use delharc::header::*;
use delharc::decode::*;
use delharc::stub_io::{self, Read};

mod sink;
use sink::SinkSum;

#[cfg(feature = "pm")]
static PM2_FILE: &[u8] = include_bytes!("decode/pm2.bin");

const TESTS_CASES: &[(&str, &[u8], CompressionMethod, u64, u32, Option<usize>)] = &[
    #[cfg(feature = "lz")]
    ("lzs", include_bytes!("decode/lzs.bin"), CompressionMethod::Lzs, 0, 0x4E46F4A1, None),
    #[cfg(feature = "lz")]
    ("lz5", include_bytes!("decode/lz5.bin"), CompressionMethod::Lz5, 1, 0x4E46F4A1, None),
    #[cfg(feature = "lh1")]
    ("lh1", include_bytes!("decode/lh1.bin"), CompressionMethod::Lh1, 1, 0x4E46F4A1, None),
    ("lh5", include_bytes!("decode/lh5.bin"), CompressionMethod::Lh5, 1, 0x4E46F4A1, None),
    ("lh6", include_bytes!("decode/lh6.bin"), CompressionMethod::Lh6, 1, 0x4E46F4A1, None),
    ("lh7", include_bytes!("decode/lh7.bin"), CompressionMethod::Lh7, 1, 0x4E46F4A1, None),
    #[cfg(feature = "pm")]
    ("pm2", PM2_FILE, CompressionMethod::Pm2,42, 0x4E46F4A1, None),
    #[cfg(feature = "pm")]
    ("pm2", PM2_FILE, CompressionMethod::Pm2,34, 0x8E2093A7, Some(18176)),

];

#[test]
fn test_decode_slice() {
    const CRC32: u32 = 0x4e46f4a1;
    let file = include_bytes!("decode/lh0.bin");
    let size = file.len();
    let mut decoder = PassthroughDecoder::new(&file[..]);
    test_decoder(&mut decoder, size, CRC32, usize::max_value());

    for (name, file, compression, offset, crc32, altsize) in TESTS_CASES {
        println!("-------------\n{:?}", name);
        let compressed_size = file.len() as u64;
        for limit in [usize::max_value(), 257, 128, 31, 3, 2, 1].iter().copied() {
            let mut slice = &file[..];
            let mut decoder = DecoderAny::new_from_compression(*compression,
                                          slice.take(compressed_size - offset));
            assert!(decoder.is_supported());
            test_decoder(&mut decoder, altsize.unwrap_or(size as usize), *crc32, limit);
            // println!("{:?}", decoder);
            slice = decoder.into_inner().into_inner();
            assert_eq!(*offset, slice.len() as u64);
        }
    }
}

fn test_decoder<R: stub_io::Read, D: Decoder<R>>(
    decoder: &mut D,
    mut target_size: usize,
    crc_validate: u32,
    buf_size: usize)
{
    let mut buffer = [0u8;1024];
    let mut sink = SinkSum::new();
    while target_size != 0 {
        let len = buffer.len().min(target_size).min(buf_size);
        let target = &mut buffer[0..len];
        decoder.fill_buffer(target).unwrap();
        sink.write_all(target).unwrap();
        target_size -= len
    }
    assert_eq!(sink.crc32.get_crc(), crc_validate);
}
