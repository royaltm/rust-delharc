/*! A library for parsing and decoding [LHA/LZH](https://en.wikipedia.org/wiki/LHA_(file_format)) archive file format.

Currently only LEVEL 0 headers are supported and a limited number of [decoders][decode].

## Supported compression methods

You may include or opt out of some of the decoders using features:

```toml
[dependencies.delharc]
version = "0.1"
default-features = false
features = ["lh1"] # select desired features
```

`lh1` and `lz` features are enabled by **default**.

| identifier | decoder            | feature | description
|------------|--------------------|---------|------------
| `-lh0-`    | PassthroughDecoder |         | no compression
| `-lh1-`    | Lh1Decoder         | lh1     | LHarc version 1, 4kB sliding window, dynamic huffman
| `-lh4-`    | Lh5Decoder         |         | LHarc version 2, 4kB sliding window, static huffman
| `-lh5-`    | Lh5Decoder         |         | LHarc version 2, 8kB sliding window, static huffman
| `-lh6-`    | Lh7Decoder         |         | LHarc version 2, 32kB sliding window, static huffman
| `-lh7-`    | Lh7Decoder         |         | LHarc version 2, 64kB sliding window, static huffman
| `-lhd-`    | PassthroughDecoder |         | an empty directory
| `-lhx-`    | LhxDecoder         | lhx     | UNLHA32.DLL method, 128-256kb sliding window, static huffman
| `-lz4-`    | PassthroughDecoder |         | no compression
| `-lzs-`    | LzsDecoder         | lz      | LArc, 2kb sliding window
| `-lz5-`    | Lz5Decoder         | lz      | LArc, 4kb sliding window
| `-pm0-`    | PassthroughDecoder |         | no compression
| `-pm1-`    | unsupported        | N/A     | PMarc, 8 Kb sliding window, static huffman
| `-pm2-`    | unsupported        | N/A     | PMarc, 4 Kb sliding window, static huffman

*/
pub mod ringbuf;
pub mod decode;
pub mod header;
pub mod bitstream;
pub mod statictree;

pub use decode::LhaDecodeReader;
pub use header::LhaHeader;
pub use header::CompressionMethod;

use std::path::Path;
use std::io;
use std::fs::File;

/// Attempts to open a file from a filesystem in read-only mode and on success returns an instance of
/// [LhaDecodeReader] with the first parsed LHA file header, ready to decode the content of the first
/// archived file.
///
/// # Errors
/// This function will return an error if an opened file is not an LHA/LZH file or the header couldn't
/// be recognized. Other errors may also be returned from [File::open] and from reading the file content.
pub fn parse_file<P: AsRef<Path>>(path: P) -> io::Result<LhaDecodeReader<File>> {
  let file = File::open(path)?;
  LhaDecodeReader::new(file)
}
