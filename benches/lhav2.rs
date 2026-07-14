//! Benchmark decompressing selected LHAv2 files from tests
use std::{fs, io, path::Path};
use core::hint::black_box;
use criterion::{
    criterion_group,
    criterion_main,
    Criterion, BenchmarkId, Throughput
};
use delharc::{decode::*, header::*};

static ARCHIVES: &[&str] = &[
    "lha213/lh5_long.lzh",
    "lha_amiga_122/lh4_long.lzh",
    "lha_unix114i/lh6_long.lzh",
    "lha_unix114i/lh7_long.lzh",
    "lharc_atari_313a/shorter.lzh",
    "unlha32/lhx_long.lzh",
];

// Build a flat tree with all the leaves at the bottom.
fn lhav2_benchmark(c: &mut Criterion) {
    let tests_dir = Path::new(file!()).join("../../tests");
    let mut buffer = vec![0u8;8192].into_boxed_slice();
    let mut group = c.benchmark_group("LHA-v2");
    for &archive in ARCHIVES {
        let mut file = match fs::read(tests_dir.join(archive) {
            Ok(f) => f,
            Err(err) => {
                eprintln!("{}: {}", archive, err);
                continue
            }
        };
        let mut file = io::Cursor::new().unwrap());
        let header = LhaHeader::read(&mut file).unwrap().unwrap();
        let compression = header.compression_method().unwrap();
        assert!(matches!(compression,
            CompressionMethod::Lh4|
            CompressionMethod::Lh5|
            CompressionMethod::Lh6|
            CompressionMethod::Lh7|
            CompressionMethod::Lhx));
        let compressed_size = header.compressed_size;
        let target_size = usize::try_from(header.original_size).unwrap();
        let file_pos = usize::try_from(file.position()).unwrap();
        let mut compressed_data = file.into_inner();
        compressed_data.drain(..file_pos);
        compressed_data.truncate(usize::try_from(compressed_size).unwrap());
        // println!("Archive: {}: {} -> {} {}",
        //     archive, compressed_size, target_size, header.parse_pathname_to_str());
        group.throughput(Throughput::Bytes(compressed_size));
        let id = BenchmarkId::from_parameter(Path::new(archive).file_name().unwrap().display());
        group.bench_with_input(id, compressed_data.as_slice(), |b, data| {
            b.iter(|| {
                let mut decoder = DecoderAny::new_from_compression(compression, data);
                let mut remaining_size = target_size;
                while remaining_size != 0 {
                    let len = buffer.len().min(remaining_size);
                    let target = &mut buffer[0..len];
                    decoder.fill_buffer(target).unwrap();
                    black_box(target);
                    remaining_size -= len
                }
            });
        });
    }
    group.finish();
}

criterion_group!(benches, lhav2_benchmark);
criterion_main!(benches);
