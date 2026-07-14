v0.8.0
* implement `core::error::Error` for error types.
* exmples: integrate examples into a single file.
* remove links to integration tests and examples form manifest file, this might have been confusing some OS packagers hence neither tests nor examples are included in the cargo package.
* Fixed LhaV2Decoder::begin_new_block to set remaining_commands only after successful tree decoding.
* Changed copy_from_history function signature, which prevciously returned a result which was always Ok.
* More unit tests added to improve coverage.
* `extend` feature introduced exposing ring buffers, static Huffman Tree and bit-stream functions.
* `extend` feature allows users to build custom `LhaV2Decoder` flavours.
* `no-unsafe-assertions` feature introduced, which remove unsafe assertions that eliminate boundary checks in critical functions; it only affects code if debug_assertions are disabled.
* lhv1/dyntree: more tests added.
* lhv1/dyntree: refactored to avoid large stack allocations, simplified the new and rebuild_tree methods.
* lhv1/dyntree: refactored to use unsafe assertions instead of unsafe slice access, which can be disabled with `no-unsafe-assertions` feature.
* statictree: more tests added, long randomized tests introduced.
* statictree fix: replace iterating open-ended ranges with an inclusive range to be able to yield MAX value.
* statictree fix: ensure tree is cleared on any building error
* statictree and dyntree: removed Display implementation when not testing.
* Added long randomized tests for all decoders.
* `CompressionMethod::is_compressed` method added and `is_directory` method takes self by value now.
* `LhaHeader` derives `PartialEq` and `Eq`.
* clippy suggested changes.

v0.7.0
* Rust edition: 2024.
* Minimum supported rust version changed to Rust 1.93 (slice::assume_init_mut).
* Deps: bitflags bumped to 2.13.
* bytemuck added to dependencies.
* replaced all uses of transmute with modern functions.
* clippy suggested changes.

v0.6.2
* Fixed a bug in LhaHeader::read that could cause integer overflow.
* Fixed a bug in LhaHeader::read that could allocate a huge memory chunk before failing.
* Added a static assert to prevent compilation on systems with usize < 32-bit.
* Added missing long_header_len check on lha_level=3 in in LhaHeader::read.
* Fixed minor warnings.
* Minimum supported rust version changed to Rust 1.65.

v0.6.1
* Fixed a bug in LhaV2Decoder::read_temp_tree that might cause a panic on a random bitstream.
* Deps: bitflags bumped to 2.5.

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
* Deps: bitflags bumped to 2.4.
* extract_nostd example added to showcase usage of nostd.
* Embedded example added to test compilation of a no-std target.

v0.5.0
* Rust edition: 2021.
* An example added to showcase usage of different reader sources.
* RingArrayBuf reworked with const generics.
* dyntree: unsafe transmute replaced with array::from_fn.
* Minimum supported rust version changed to Rust 1.63 (array::from_fn).
* Deps: bitflags updated to 2.3.
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
