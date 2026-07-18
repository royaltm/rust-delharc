//! Benchmark decompressing selected PMarc v1 files from tests
use std::{fs, io, path::Path, time::Duration};
use core::hint::black_box;
use criterion::{
    criterion_group,
    criterion_main,
    Criterion, BenchmarkId, Throughput
};
use delharc::{decode::*, header::*};

static ARCHIVES: &[&str] = &[
    "pmarc124/pm1_long.pma",
    "pmarc2/long.pma",
];

fn pmarc1_benchmark(c: &mut Criterion) {
    let tests_dir = Path::new(file!()).join("../../tests");
    let mut buffer = vec![0u8;8192].into_boxed_slice();
    let mut group = c.benchmark_group("PMarc");
    group.sample_size(1000);
    group.measurement_time(Duration::from_secs(15));
    for &archive in ARCHIVES {
        let file = match fs::read(tests_dir.join(archive)) {
            Ok(f) => f,
            Err(err) => {
                eprintln!("{}: {}", archive, err);
                continue
            }
        };
        let mut file = io::Cursor::new(file);
        let header = LhaHeader::read(&mut file).unwrap().unwrap();
        let compression = header.compression_method().unwrap();
        assert!(matches!(compression, CompressionMethod::Pm1|
                                      CompressionMethod::Pm2));
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

criterion_group!(benches, pmarc1_benchmark);
criterion_main!(benches);
