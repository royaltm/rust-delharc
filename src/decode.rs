//! # Decoding algorithms.
use std::io;
use crc::{crc16, Hasher16};
use crate::header::{CompressionMethod, LhaHeader};

#[cfg(feature = "lz")]
mod lzs;
#[cfg(feature = "lz")]
mod lz5;
#[cfg(feature = "lh1")]
mod lhv1;
mod lhv2;

#[cfg(feature = "lz")]
pub use lzs::*;
#[cfg(feature = "lz")]
pub use lz5::*;
#[cfg(feature = "lh1")]
pub use lhv1::*;
pub use lhv2::*;

/// The trait implemented by decoders.
pub trait Decoder<R> {
    /// Unwraps and returns the inner reader.
    fn into_inner(self) -> R;
    /// Fills the whole `buf` with decoded data.
    ///
    /// The caller should be aware of how large buffer can be provided to not exceed the size
    /// of the decompressed file. Otherwise it will most likely result in an unexpected EOF error.
    fn fill_buffer(&mut self, buf: &mut[u8]) -> io::Result<()>;
}

/// This is the LHA/LZH file format decoding reader.
///
/// It can parse subsequent headers from the underlying data stream and decompress files.
///
/// To read the current archived file's content use the [io::Read] trait methods on the instance of this type.
///
/// After parsing the LHA header, a decompressed content of a file can be simply read from the `LhaDecodeReader`,
/// which decompresses it using a proper decoder, designated in the header, while reading data from the
/// underlying stream.
pub struct LhaDecodeReader<R> {
    header: LhaHeader,
    crc: crc16::Digest,
    output_length: usize,
    decoder: Option<DecoderAny<io::Take<R>>>
}

/// An empty decoder for storage only methods.
#[derive(Debug)]
pub struct PassthroughDecoder<R> {
    inner: R
}

#[non_exhaustive]
#[derive(Debug)]
pub enum DecoderAny<R> {
    PassthroughDecoder(PassthroughDecoder<R>),
    #[cfg(feature = "lz")]
    LzsDecoder(LzsDecoder<R>),
    #[cfg(feature = "lz")]
    Lz5Decoder(Lz5Decoder<R>),
    #[cfg(feature = "lh1")]
    Lh1Decoder(Lh1Decoder<R>),
    Lh4Decoder(Lh5Decoder<R>),
    Lh5Decoder(Lh5Decoder<R>),
    Lh6Decoder(Lh7Decoder<R>),
    Lh7Decoder(Lh7Decoder<R>),
    #[cfg(feature = "lhx")]
    LhxDecoder(LhxDecoder<R>),
}

macro_rules! decoder_any_dispatch {
    (($model:expr)($($spec:tt)*) => $expr:expr) => {
        match $model {
            DecoderAny::PassthroughDecoder($($spec)*) => $expr,
            #[cfg(feature = "lz")]
            DecoderAny::LzsDecoder($($spec)*) => $expr,
            #[cfg(feature = "lz")]
            DecoderAny::Lz5Decoder($($spec)*) => $expr,
            #[cfg(feature = "lh1")]
            DecoderAny::Lh1Decoder($($spec)*) => $expr,
            DecoderAny::Lh4Decoder($($spec)*)|
            DecoderAny::Lh5Decoder($($spec)*) => $expr,
            DecoderAny::Lh6Decoder($($spec)*)|
            DecoderAny::Lh7Decoder($($spec)*) => $expr,
            #[cfg(feature = "lhx")]
            DecoderAny::LhxDecoder($($spec)*) => $expr,
        }
    };
}

/// A default implementation creates an instance of `LhaDecodeReader<R>` with no reader present and
/// with a phony header.
impl<R: io::Read> Default for LhaDecodeReader<R> {
    fn default() -> Self {
        LhaDecodeReader {
            header: Default::default(),
            crc: crc16::Digest::new(crc16::USB),
            output_length: 0,
            decoder: None
        }
    } 
}

impl<R: io::Read> LhaDecodeReader<R> {
    /// Creates a new instance of `LhaDecodeReader<R>` after reading and parsing the first header from source.
    ///
    /// Pass the stream reader as an instance value or a mutable reference to it.
    ///
    /// # Errors
    /// Returns an error if the header could not be read or parsed.
    pub fn new(mut rd: R) -> io::Result<LhaDecodeReader<R>> {
        let header = LhaHeader::read(rd.by_ref())?
                    .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "header missing"))?;
        let limited_rd = rd.take(header.compressed_size as u64);
        let decoder = DecoderAny::new_from_compression(header.compression, limited_rd);
        if decoder.is_none() {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "unsupported compression method"))
        }
        let crc = crc16::Digest::new(crc16::USB);
        Ok(LhaDecodeReader {
            header,
            crc,
            output_length: 0,
            decoder
        })
    }
    /// Attempts to read the first file header from a new source stream and initializes a decoder returning
    /// `Ok(true)` on success. Returns `Ok(false)` if there are no more headers in the stream.
    ///
    /// Pass the stream reader as an instance value or a mutable reference to it.
    ///
    /// Regardles of the retuned boolean value, the inner reader is being always replaced with `rd`.
    ///
    /// # Errors
    /// Returns an error if the header could not be read or parsed. In this instance the inner stream
    /// reader is not being replaced by a new one.
    pub fn with_new(&mut self, mut rd: R) -> io::Result<bool> {
        let res = if let Some(header) = LhaHeader::read(rd.by_ref())? {
            let limited_rd = rd.take(header.compressed_size as u64);
            let decoder = DecoderAny::new_from_compression(header.compression, limited_rd);
            if decoder.is_none() {
                return Err(io::Error::new(io::ErrorKind::InvalidData, "unsupported compression method"))
            }
            self.decoder = decoder;
            self.header = header;
            true
        }
        else {
            let decoder = PassthroughDecoder::new(rd.take(0));
            self.decoder = Some(DecoderAny::PassthroughDecoder(decoder));
            false
        };
        self.crc.reset();
        self.output_length = 0;
        Ok(res)
    }
    /// Assigns a header and the decoder externally parsed.
    ///
    /// It is up to the caller to make sure the decoder and the header are correct.
    ///
    /// This method assumes the file will be read and decoded from its beginning.
    pub fn with_header_and_decoder(&mut self, header: LhaHeader, decoder: DecoderAny<io::Take<R>>) {
        self.decoder = Some(decoder);
        self.header = header;
        self.crc.reset();
        self.output_length = 0;
    }
    /// Attempts to parse the next file's header.
    ///
    /// The remaining content of the previous file is being skipped if the current file's content has not been
    /// read entirely.
    ///
    /// On success returns `Ok(true)` if the next header has been read and parsed successfully.
    /// If there are no more headers returns `Ok(false)`.
    ///
    /// # Errors
    /// Returns an error if the header could not be read or parsed.
    ///
    /// # Panics
    /// Panics if called when the underlying stream reader has been already taken.
    pub fn next_file(&mut self) -> io::Result<bool> {
        let mut limited_rd = self.decoder.take().expect("decoder not empty").into_inner();
        if limited_rd.limit() != 0 {
            io::copy(&mut limited_rd, &mut io::sink())?;
        }
        self.with_new(limited_rd.into_inner())
    }
    /// Returns a reference to the last parsed file's [LhaHeader].
    pub fn header(&self) -> &LhaHeader {
        &self.header
    }
    /// Unwraps the underlying stream reader and returns it.
    ///
    /// # Panics
    /// Panics if the reader has been already taken.
    pub fn into_inner(self) -> R {
        self.decoder.expect("decoder not empty").into_inner().into_inner()
    }
    /// Takes the inner stream reader value out of the decoder, leaving a none in its place.
    ///
    /// After this call, reading from this instance will result in panic.
    pub fn take_inner(&mut self) -> Option<R> {
        self.header.original_size = 0;
        self.output_length = 0;
        self.crc.reset();
        self.decoder.take().map(|decoder| decoder.into_inner().into_inner())
    }
    /// Returns the number of remaining bytes to be read.
    pub fn len(&self) -> usize {
        self.header.original_size as usize - self.output_length
    }
    /// Returns `true` if the current file has been finished reading.
    pub fn is_empty(&self) -> usize {
        self.header.original_size as usize - self.output_length
    }
    /// Returns `true` if an underlying stream reader is present in the decoder.
    pub fn is_present(&self) -> bool {
        self.decoder.is_some()
    }
    /// Returns `true` if an underlying stream reader is absent in the decoder.
    ///
    /// Reading from such decoder will result in panic.
    pub fn is_absent(&self) -> bool {
        self.decoder.is_none()
    }
    /// Returns `true` if the computed CRC-16 matches the checksum in the header.
    ///
    /// This should be called after the whole file has been read.
    pub fn crc_is_ok(&self) -> bool {
        self.crc.sum16() == self.header.crc
    }
    /// Returns CRC-16 checksum if the computed checksum matches the one in the header.
    /// Otherwise returns an error.
    ///
    /// This should be called after the whole file has been read.
    pub fn crc_check(&self) -> io::Result<u16> {
        if self.crc_is_ok() {
            Ok(self.header.crc)
        }
        else {
            Err(io::Error::new(io::ErrorKind::InvalidData, "crc16 mismatch"))
        }
    }
}

impl<R: io::Read + 'static> io::Read for LhaDecodeReader<R> {
    fn read(&mut self, buf: &mut[u8]) -> io::Result<usize> {
        let len = buf.len().min(self.header.original_size as usize - self.output_length);
        let target = &mut buf[0..len];
        self.decoder.as_mut().unwrap().fill_buffer(target)?;
        self.output_length += len;
        self.crc.write(target);
        Ok(len)
    }
}

impl<R: io::Read> DecoderAny<R> {
    /// Creates an instance of DecoderAny<R> from the given compression method and a stream reader.
    pub fn new_from_compression(
            compression: CompressionMethod,
            rd: R
        ) -> Option<Self>
    {
        Some(match compression {
            CompressionMethod::Pm0|
            CompressionMethod::Lz4|
            CompressionMethod::Lh0 => DecoderAny::PassthroughDecoder(PassthroughDecoder::new(rd)),
            #[cfg(feature = "lz")]
            CompressionMethod::Lzs => DecoderAny::LzsDecoder(LzsDecoder::new(rd)),
            #[cfg(feature = "lz")]
            CompressionMethod::Lz5 => DecoderAny::Lz5Decoder(Lz5Decoder::new(rd)),
            #[cfg(feature = "lh1")]
            CompressionMethod::Lh1 => DecoderAny::Lh1Decoder(Lh1Decoder::new(rd)),
            CompressionMethod::Lh4 => DecoderAny::Lh4Decoder(Lh5Decoder::new(rd)),
            CompressionMethod::Lh5 => DecoderAny::Lh5Decoder(Lh5Decoder::new(rd)),
            CompressionMethod::Lh6 => DecoderAny::Lh6Decoder(Lh7Decoder::new(rd)),
            CompressionMethod::Lh7 => DecoderAny::Lh7Decoder(Lh7Decoder::new(rd)),
            #[cfg(feature = "lhx")]
            CompressionMethod::Lhx => DecoderAny::LhxDecoder(LhxDecoder::new(rd)),
            _ => return None
        })
    }
}

impl<R: io::Read> Decoder<R> for DecoderAny<R> {
    fn into_inner(self) -> R {
        decoder_any_dispatch!((self)(decoder) => decoder.into_inner())
    }

    #[inline]
    fn fill_buffer(&mut self, buf: &mut[u8]) -> io::Result<()> {
        decoder_any_dispatch!((self)(decoder) => decoder.fill_buffer(buf))
    }
}

impl<R: io::Read> PassthroughDecoder<R> {
    pub fn new(inner: R) -> Self {
        PassthroughDecoder { inner }
    }
}

impl<R: io::Read> Decoder<R> for PassthroughDecoder<R> {
    fn into_inner(self) -> R {
        self.inner
    }

    #[inline]
    fn fill_buffer(&mut self, buf: &mut[u8]) -> io::Result<()> {
        self.inner.read_exact(buf)
    }
}
