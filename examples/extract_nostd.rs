#[cfg(not(feature = "std"))]
use delharc::{LhaError, LhaResult, Read, UnexpectedEofError};

#[cfg(not(feature = "std"))]
fn extract_to_stdout<R: Read, P: AsRef<str>>(
        mut lha_reader: delharc::LhaDecodeReader<R>,
        matching_path: P
    ) -> LhaResult<bool, R>
    where R::Error: core::fmt::Debug
{
    let mut buf = [0u8; 1024];
    loop {
        let header = lha_reader.header();
        let filename = header.parse_pathname_to_str();

        eprintln!("  Path: {:?} modified: {} ", filename, header.parse_last_modified());
        eprintln!("----------------------------------------------------------------");

        if filename.ends_with(matching_path.as_ref()) {
            if lha_reader.is_decoder_supported() {
                loop {
                    let n = lha_reader.read_all(&mut buf[..])?;
                    if n == 0 {
                        break;
                    }
                    print!("{}", core::str::from_utf8(&buf[..n]).unwrap());
                }
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

#[allow(unused_macros)]
macro_rules! archive_name {
    () => { "tests/lha_amiga_212/lh6.lzh" };
}

#[cfg(not(feature = "std"))]
fn main() -> Result<(), LhaError<UnexpectedEofError>> {
    const FILE_MATCH: &str = "gpl-2";

    eprintln!("");
    eprintln!("================================================================");
    eprintln!("  Extracting from &[u8]...");
    eprintln!("================================================================");
    const SLICE: &[u8] = include_bytes!(concat!("../", archive_name!()));
    let lha_reader = delharc::LhaDecodeReader::new(SLICE)?;
    extract_to_stdout(lha_reader, FILE_MATCH)?;

    Ok(())
}


#[cfg(feature = "std")]
fn main() {
    eprintln!("Re-run with --no-default-features");
}
