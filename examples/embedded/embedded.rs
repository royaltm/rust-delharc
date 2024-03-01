#![no_std]
#![no_main]
extern crate alloc;
use alloc::{format, boxed::Box, vec::Vec};
use core::mem::MaybeUninit;
use panic_halt as _;
use cortex_m_rt::entry;
use embedded_alloc::Heap;

use delharc::*;

#[no_mangle]
#[global_allocator]
static HEAP: Heap = Heap::empty();
const HEAP_SIZE: usize = 32*1024;

static COMPRESSED_1: &[u8] = include_bytes!("../../tests/lha_amiga_212/lh1.lzh");
static COMPRESSED_6: &[u8] = include_bytes!("../../tests/lha_amiga_212/lh6.lzh");
const UNCOMPRESSED_SIZE: usize = 18092;
const CRC16: u16 = 0xA33A;
const FILE_MATCH: &str = "gpl-2";
const TIMESTAMP: &str = "1980-06-12 21:03:18";

fn extract_check<R: Read, P: AsRef<str>>(
        mut lha_reader: delharc::LhaDecodeReader<R>,
        matching_path: P
    ) -> LhaResult<bool, R>
    where R::Error: core::fmt::Debug
{
    let mut buf: Box<[u8]> = {
      let mut vec = Vec::new();
      vec.resize(UNCOMPRESSED_SIZE, 0u8);
      vec.into_boxed_slice()
    };
    loop {
        let header = lha_reader.header();
        let filename = header.parse_pathname_to_str();

        let last_modified = format!("{}", header.parse_last_modified());
        assert_eq!(&last_modified, TIMESTAMP);
        assert_eq!(&filename, FILE_MATCH);

        if filename.ends_with(matching_path.as_ref()) {
            if lha_reader.is_decoder_supported() {
                let n = lha_reader.read_all(&mut buf)?;
                assert_eq!(n, UNCOMPRESSED_SIZE);
                assert_eq!(CRC16, lha_reader.crc_check()?);
                return Ok(true)
            }
        }

        if !lha_reader.next_file()? {
            break;
        }
    }

    Ok(false)
}

#[entry]
fn main() -> ! {
  {
      #[no_mangle]
      static mut HEAP_MEM: [MaybeUninit<u8>; HEAP_SIZE] = [MaybeUninit::uninit(); HEAP_SIZE];
      unsafe { HEAP.init(HEAP_MEM.as_ptr() as usize, HEAP_SIZE) }
  }

  let lha_reader = delharc::LhaDecodeReader::new(COMPRESSED_1).unwrap();
  assert!(extract_check(lha_reader, FILE_MATCH).unwrap());

  let lha_reader = delharc::LhaDecodeReader::new(COMPRESSED_6).unwrap();
  assert!(extract_check(lha_reader, FILE_MATCH).unwrap());

  loop {}
}
