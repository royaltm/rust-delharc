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
