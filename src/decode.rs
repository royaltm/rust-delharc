//! # Decoding algorithms.
use std::io;

use crate::crc::Crc16;
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
    crc: Crc16,
    output_length: u64,
    decoder: Option<DecoderAny<io::Take<R>>>
}

/// An empty decoder for storage only methods.
#[derive(Debug)]
pub struct PassthroughDecoder<R> {
    inner: R
}

/// A decoder used when compression method is unsupported.
/// Reading from it will always produce an error.
#[derive(Debug)]
pub struct UnsupportedDecoder<R> {
    inner: R
}

#[non_exhaustive]
#[derive(Debug)]
pub enum DecoderAny<R> {
    PassthroughDecoder(PassthroughDecoder<R>),
    UnsupportedDecoder(UnsupportedDecoder<R>),
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
            DecoderAny::UnsupportedDecoder($($spec)*) => $expr,
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
            crc: Crc16::default(),
            output_length: 0,
            decoder: None
        }
    } 
}

impl<R: io::Read> LhaDecodeReader<R> {
    /// Creates a new instance of `LhaDecodeReader<R>` after reading and parsing the first header from source.
    ///
    /// Provide an instance of the stream reader.
    ///
    /// # Errors
    /// Returns an error if the header could not be read or parsed.
    pub fn new(mut rd: R) -> io::Result<LhaDecodeReader<R>> {
        let header = LhaHeader::read(rd.by_ref())?
                    .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "header missing"))?;
        let limited_rd = rd.take(header.compressed_size as u64);
        let decoder = match header.compression_method() {
            Ok(compression) => DecoderAny::new_from_compression(compression, limited_rd),
            Err(..) => DecoderAny::UnsupportedDecoder(UnsupportedDecoder::new(limited_rd))
        };
        let crc = Crc16::default();
        Ok(LhaDecodeReader {
            header,
            crc,
            output_length: 0,
            decoder: Some(decoder)
        })
    }
    /// Attempts to read the first file header from a new source stream and initializes a decoder returning
    /// `Ok(true)` on success. Returns `Ok(false)` if there are no more headers in the stream.
    ///
    /// Provide an instance of the stream reader.
    ///
    /// Regardles of the retuned boolean value, the inner reader is being always replaced with `rd`.
    ///
    /// # Errors
    /// Returns an error if the header could not be read or parsed. In this instance the inner stream
    /// reader is not being replaced by a new one.
    pub fn with_new(&mut self, mut rd: R) -> io::Result<bool> {
        let res = if let Some(header) = LhaHeader::read(rd.by_ref())? {
            let limited_rd = rd.take(header.compressed_size as u64);
            let decoder = match header.compression_method() {
                Ok(compression) => DecoderAny::new_from_compression(compression, limited_rd),
                Err(..) => DecoderAny::UnsupportedDecoder(UnsupportedDecoder::new(limited_rd))
            };
            self.decoder = Some(decoder);
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
    /// Assigns externally parsed header and decoder to this instance of `LhaDecodeReader<R>`.
    ///
    /// It is up to the caller to make sure the decoder and the header are matching each other.
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
    pub fn len(&self) -> u64 {
        self.header.original_size - self.output_length
    }
    /// Returns `true` if the current file has been finished reading.
    pub fn is_empty(&self) -> bool {
        self.header.original_size == self.output_length
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
        self.crc.sum16() == self.header.file_crc
    }
    /// Returns CRC-16 checksum if the computed checksum matches the one in the header.
    /// Otherwise returns an error.
    ///
    /// This should be called after the whole file has been read.
    pub fn crc_check(&self) -> io::Result<u16> {
        if self.crc_is_ok() {
            Ok(self.header.file_crc)
        }
        else {
            Err(io::Error::new(io::ErrorKind::InvalidData, "crc16 mismatch"))
        }
    }
    /// Returns `true` if the current file's decoder is supported.
    /// If this method returns `false`, trying to read from the decoder will result in an error.
    /// In this instance it is still ok to skip to the next file.
    pub fn is_decoder_supported(&self) -> bool {
        self.decoder.as_ref().map(|d| d.is_supported()).unwrap_or(false)
    }
}

impl<R: io::Read + 'static> io::Read for LhaDecodeReader<R> {
    fn read(&mut self, buf: &mut[u8]) -> io::Result<usize> {
        let len = buf.len().min((self.header.original_size - self.output_length) as usize);
        let target = &mut buf[0..len];
        self.decoder.as_mut().unwrap().fill_buffer(target)?;
        self.output_length += len as u64;
        self.crc.digest(target);
        Ok(len)
    }
}

impl<R: io::Read> DecoderAny<R> {
    /// Creates an instance of DecoderAny<R> from the given compression method and a stream reader.
    pub fn new_from_compression(
            compression: CompressionMethod,
            rd: R
        ) -> Self
    {
        match compression {
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
            _ => DecoderAny::UnsupportedDecoder(UnsupportedDecoder::new(rd))
        }
    }
    /// Returns `true` if the decoder is able to decode the file content.
    pub fn is_supported(&self) -> bool {
        match self {
            DecoderAny::UnsupportedDecoder(..) => true,
            _ => false
        }
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

impl<R: io::Read> UnsupportedDecoder<R> {
    pub fn new(inner: R) -> Self {
        UnsupportedDecoder { inner }
    }
}

impl<R: io::Read> Decoder<R> for UnsupportedDecoder<R> {
    fn into_inner(self) -> R {
        self.inner
    }

    #[inline]
    fn fill_buffer(&mut self, _buf: &mut[u8]) -> io::Result<()> {
        Err(io::Error::new(io::ErrorKind::InvalidData, "unsupported compression method"))
    }
}
