v0.6.1
* Fixed a bug in LhaV2Decoder::read_temp_tree that might cause a panic on a random bitstream.
* Deps: bitflags upgraded to 2.5.

v0.6.0
* no-std is enabled in the absence of the std feature.
* Breaking changes for exported types and function signatures:
  - Generic types and methods depending on std::io::Read now depend on stub_io::Read.
  - Methods previously returning std::io::Error return LhaError instead.
  - LhaHeader::read signature argument changed to &mut R.
* LhaHeader::parse_pathname_to_string added.
* LhaDecodeReader::next_file_with_sink added.
* TimestampResult::to_local is only available with std feature.
* LhaHeader::parse_pathname is only available with std feature.
* delharc::parse_file is only available with std feature.
* Deps: bitflags upgraded to 2.4.
* extract_nostd example added to showcase usage of nostd.
* Embedded example added to test compilation of a no-std target.

v0.5.0
* Rust edition: 2021
* An example added to showcase usage of different reader sources.
* RingArrayBuf reworked with const generics.
* dyntree: unsafe transmute replaced with array::from_fn.
* Minimum supported rust version changed to Rust 1.63 (array::from_fn).
* Deps: bitflags upgraded to 2.3.
* License files added.

v0.4.0
* CI: migration to Github Actions.
* Tests: pass all tests regardless of selected features.
* Minimum supported rust version changed to Rust 1.46 (const fn improvements).
* Deps: bitflags updated to 1.3, dev-deps updated.

v0.3.0
* LhaHeader::parse_comment.
* LhaHeader::parse_pathname returns the file name up to a nul character with Amiga archives.

v0.2.2
* Removed unnecessary static lifetime constraint on the inner reader of LhaDecodeReader.

v0.2.1
* Fixed result of LhaHeader::is_supported and LhaDecodeReader::is_decoder_supported.

v0.2.0
* Added a missing Debug trait implementation for LhaDecodeReader.
* Some methods of LhaDecodeReader now return an error variant as LhaDecodeError, so the stream source can be retrieved.
* Fixed a minor bug in the documentation.

v0.1.0
* The initial release.
