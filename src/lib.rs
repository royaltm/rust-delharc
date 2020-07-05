/*! A library for parsing and extracting content of [LHA/LZH](https://en.wikipedia.org/wiki/LHA_(file_format)) archives.

This library is for easy parsing of LHA headers and allows to read files compressed with some of the
methods used by the archive format.

This library does not provide high level methods for creating files or directories from the extracted archives.

There are many extensions to the base LHA headers, used by many different archive programs, in many different
operating systems. This library only allows for parsing some basic properties of the archived files, such as
file path names and last modification timestamps.

The [LhaHeader] exposes properties and methods to inspect the raw content of header extensions, as well as
extended header data and may be explored by the user program in case extra archive properties are needed to be read.

LHA header levels: 0, 1, 2 and 3 are supported.

## Compression methods

You may include or opt out of some of the decoders:

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
| `-lhx-`    | LhxDecoder         | lhx     | UNLHA32.DLL method, 128-512kb sliding window, static huffman
| `-lz4-`    | PassthroughDecoder |         | no compression
| `-lzs-`    | LzsDecoder         | lz      | LArc, 2kb sliding window
| `-lz5-`    | Lz5Decoder         | lz      | LArc, 4kb sliding window
| `-pm0-`    | PassthroughDecoder |         | no compression
| `-pm1-`    | unsupported        | N/A     | PMarc, 8 Kb sliding window, static huffman
| `-pm2-`    | unsupported        | N/A     | PMarc, 4 Kb sliding window, static huffman

## Example

```no_run
use std::{io, path::Path};

fn extract_to_stdout<P: AsRef<Path>>(
        archive_name: P,
        matching_path: P
    ) -> io::Result<bool>
{
    let mut lha_reader = delharc::parse_file(archive_name.as_ref())?;
    loop {
        let header = lha_reader.header();
        let filename = header.parse_pathname();

        eprintln!("Path: {:?} modified: {} ", filename, header.parse_last_modified());

        if filename.ends_with(matching_path.as_ref()) {
            if lha_reader.is_decoder_supported() {
                let stdout = io::stdout();
                let mut handle = stdout.lock();
                io::copy(&mut lha_reader, &mut handle)?;
                lha_reader.crc_check()?;
                return Ok(true)
            }
            else if header.is_directory() {
                eprintln!("skipping: an empty directory");
            }
            else {
                eprintln!("skipping: has unsupported compression method");
            }
        }

        if !lha_reader.next_file()? {
            break;
        }
    }

    Ok(false)
}
```
*/
// http://archive.gamedev.net/archive/reference/articles/article295.html
pub mod crc;
pub mod decode;
pub mod header;
pub(crate) mod ringbuf;
pub(crate) mod bitstream;
pub(crate) mod statictree;

pub use decode::LhaDecodeReader;
pub use header::{
    LhaHeader, CompressionMethod, OsType, TimestampResult, MsDosAttrs
};

use std::path::Path;
use std::io;
use std::fs::File;

/// Attempts to open a file from a filesystem in read-only mode and on success returns an instance of
/// [LhaDecodeReader] with the first parsed LHA file header, ready to decode the content of the first
/// archived file.
///
/// # Errors
/// This function will return an error if an opened file is not an LHA/LZH file or the header couldn't
/// be recognized. Other errors may also be returned from [File::open] and from attempts to read the file.
pub fn parse_file<P: AsRef<Path>>(path: P) -> io::Result<LhaDecodeReader<File>> {
  let file = File::open(path)?;
  LhaDecodeReader::new(file)
}
