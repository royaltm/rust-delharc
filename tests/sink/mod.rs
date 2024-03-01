#[cfg(feature = "std")]
use std::io;
use crc_any::{CRCu32, CRCu16};

pub struct SinkSum {
    pub length: u64,
    pub crc16: CRCu16,
    pub crc32: CRCu32
}

impl SinkSum {
    pub fn new() -> Self {
        SinkSum {
            length: 0,
            crc16: CRCu16::crc16(),
            crc32: CRCu32::crc32()
        }
    }

    fn update(&mut self, buf: &[u8]) -> usize {
        self.length += buf.len() as u64;
        self.crc16.digest(buf);
        self.crc32.digest(buf);
        buf.len()
    }
}

#[cfg(feature = "std")]
impl io::Write for SinkSum {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.update(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[cfg(not(feature = "std"))]
impl SinkSum {
    pub fn write_all(&mut self, buf: &[u8]) -> Result<usize, ()> {
        self.update(buf);
        Ok(buf.len())
    }
}
