//! Benchmark decompressing selected files from the Aminet
use std::{fs, io::{self, Read, Seek}, path::Path, time::Duration};
use core::hint::black_box;
use criterion::{
    criterion_group,
    criterion_main,
    Criterion, BenchmarkId, Throughput
};
use delharc::{decode::*, header::*};

const AMINET_URL: &str = "https://aminet.net/";
static AMINET: &[(&str, &str)] = &[
    ("mods/8voic/AChristmasKiss.lha", "AChristmasKiss"),
    ("mods/8voic/Afterlife.lha", "Afterlife"),
    ("dev/misc/am-git.lha", "am-git/am-git"),
    ("util/misc/LumiPass_1.2.lha", "LumiPass/LumiPass.lumidict"),
    ("game/think/ttycity.lha", "ttycity-1.0.1/ttycity"),
];

fn find_file<R: io::Read>(file: R, name: &str) -> io::Result<(LhaHeader, Box<[u8]>)> {
    let mut lha_reader = LhaDecodeReader::new(file)?;
    loop {
        let header = lha_reader.header();
        let filename = header.parse_pathname_to_str();
        let compression = header.compression_method()?;
        if filename == name {
            let os = header.parse_os_type().map(<&str>::from).unwrap_or("unknown");
            println!("{:10} {:10} {} <{}> {:8} {}",
                header.compressed_size,
                header.original_size,
                compression,
                header.level,
                os,
                filename);
            if lha_reader.is_decoder_supported() {
                let (header, decoder) = lha_reader.into_parts();
                let mut bytes = Vec::new();
                decoder.unwrap().into_inner().into_inner()
                                .take(header.compressed_size)
                                .read_to_end(&mut bytes)?;
                let bytes = bytes.into_boxed_slice();
                return Ok((header, bytes))
            }
            else if header.is_directory() {
                return Err(io::Error::other("matched a directory"))
            }
            else {
                return Err(io::Error::other("matched unsupported compression method"))
            }
        }
        if !lha_reader.next_file()? {
            return Err(io::ErrorKind::NotFound.into())
        }
    }
}

fn fetch_cached(archive: &str, file_name: &str) -> io::Result<(LhaHeader, Box<[u8]>)> {
    let mut file_path = Path::new(file!()).join("../cache");
    file_path.push(archive);
    match fs::File::open(&file_path) {
        Ok(file) => {
            println!("--hit--: {}", archive);
            return find_file(file, file_name)
        }
        Err(err) if err.kind() == io::ErrorKind::NotFound => {}
        Err(err) => return Err(err)
    };
    println!("--miss--: {}", archive);
    let file_dir = file_path.parent().unwrap();
    fs::create_dir_all(file_dir)?;
    let mut url = AMINET_URL.to_string();
    url.push_str(archive);
    println!("fetching: {}", url);
    let mut web_file = gofer::open_buffered(url)?;
    let mut cache_file = fs::File::create_new(file_path)?;
    let copied = io::copy(&mut web_file, &mut cache_file)?;
    drop(web_file);
    println!("cached: ({}) {}", copied, archive);
    cache_file.rewind()?;
    find_file(cache_file, file_name)
}

fn aminet_benchmark(c: &mut Criterion) {
    let mut buffer = vec![0u8;8192].into_boxed_slice();
    let mut group = c.benchmark_group("Aminet");
    group.sample_size(100);
    group.measurement_time(Duration::from_secs(15));
    for &(archive, file_name) in AMINET {
        println!("------------------------\n{}: {}", archive, file_name);
        let (header, compressed_data) = fetch_cached(archive, file_name).unwrap();
        let compression = header.compression_method().unwrap();
        assert!(compression.is_compressed(), "data not packed: {}", compression);
        let compressed_size = header.compressed_size;
        let target_size = usize::try_from(header.original_size).unwrap();
        group.throughput(Throughput::Bytes(compressed_size));
        let id = BenchmarkId::from_parameter(file_name);
        group.bench_with_input(id, &*compressed_data, |b, data| {
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

criterion_group!(benches, aminet_benchmark);
criterion_main!(benches);
