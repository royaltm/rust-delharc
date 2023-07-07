use std::{io, fs, path::Path};

fn extract_to_stdout<R: io::Read, P: AsRef<Path>>(
        mut lha_reader: delharc::LhaDecodeReader<R>,
        matching_path: P
    ) -> io::Result<bool>
{
    loop {
        let header = lha_reader.header();
        let filename = header.parse_pathname();

        eprintln!("  Path: {:?} modified: {} ", filename, header.parse_last_modified());
        eprintln!("----------------------------------------------------------------");

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

macro_rules! archive_name {
    () => { "tests/lha_amiga_212/lh6.lzh" };
}

fn main() -> io::Result<()> {
    const ARCHIVE_NAME: &str = archive_name!();
    const FILE_MATCH: &str = "gpl-2";

    eprintln!("================================================================");
    eprintln!("  Extracting from io::File...");
    eprintln!("================================================================");
    let lha_reader = delharc::parse_file(ARCHIVE_NAME)?;
    extract_to_stdout(lha_reader, FILE_MATCH)?;

    eprintln!("");
    eprintln!("================================================================");
    eprintln!("  Extracting from &[u8]...");
    eprintln!("================================================================");
    const SLICE: &[u8] = include_bytes!(concat!("../", archive_name!()));
    let lha_reader = delharc::LhaDecodeReader::new(SLICE)?;
    extract_to_stdout(lha_reader, FILE_MATCH)?;

    eprintln!("");
    eprintln!("================================================================");
    eprintln!("  Extracting from Cursor<Vec<u8>>...");
    eprintln!("================================================================");
    let vec = fs::read(ARCHIVE_NAME)?;
    let lha_reader = delharc::LhaDecodeReader::new(io::Cursor::new(vec))?;
    extract_to_stdout(lha_reader, FILE_MATCH)?;

    Ok(())
}
