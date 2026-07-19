v0.8.0

General changes:
* Minimum supported rust version changed to Rust 1.95 (if-let guards in matches).
* Implemented PMarc's archiver `-pm1-` and `-pm2-` compression methods decoders.
* More unit tests to improve coverage and long randomized tests added.
* More archives imported from `lhasa` project for integrating testing.
* Benchmarking added to guide the changes in the critical functions.
* `examples`: merged `extract` and `extract_nostd` into a single file.
* `examples`: `list_files` added.
*  clippy suggested changes.

Breaking changes:
* `Decoder::Error` now requires `core::error::Error` instead of `fmt::Debug`.
* `LhaError` changed to include new error objects `LhaHeaderError` and `DecompressionError` instead of static strings.
* The way level 0 extended area is parsed by `LhaHeader::read()` has changed. Previously the first byte of the extended area on level 0 was unconditionally treated as OS ID byte and removed from the extended area. Now the whole content of the extended area is stored in the `extended_area` property and the first byte of the `extended_area` is only consulted by the `parse_os_type()` method to check for selected OS identifiers.
* `CompressionMethod::is_directory()` now takes `self` by value.
* Fixed, but also altered the way `LhaHeader::parse_pathname()` and `parse_pathname_to_str()` treat files which have none or empty `filename` field but non-empty `directory` field. In this instance a trailing directory separator is appended to the parsed path name. If a level 0 or 1 `filename` entry contains a trailing directory separator and there is no `directory` field, the directory separator will be present at the end of the parsed path. This change helps to detect if an entry is a directory name rather than a file name.

New features:
* `pm` includes PMarc archiver decoders, enabled now by default.
* `extend` expose previously internal implementations of a ring buffer, a static Huffman Tree and a bit-stream reader; allows users to build custom `LhaV2Decoder` variants.
* `no-unsafe-assertions` remove unsafe assertions that eliminate boundary checks in critical functions; it only affects code if `debug_assertions` are disabled. if `debug_assertions` are enabled all unsafe assertions become the run-time assertions.
* `fast-tree-build` enables more complex but faster static tree building method.

Improvements:
* Error objects implement `core::error::Error`.
* Replace potential temporary large stack allocations in decoders with `bytemuck::zeroed_box()`.
* Reimplemented and simplified tree building methods of the dynamic Huffman Tree used by `lhv1` decoder.
* Static and dynamic Huffman Tree implementations refactored to use unsafe assertions instead of unsafe slice accesses, which can be disabled with the `no-unsafe-assertions` feature.
* An alternative tree building method added to the static Huffman Tree object, gated under the `fast-tree-build` feature.
* Improved the ring buffer implementation.
* Error messages changed to better reflect causes of errors.
* `LhaError` to `std::io::Error` conversion includes the orignal `LhaError` encapsulation variant.

Additions:
* New error objects: `LhaHeaderError`, `DecompressionError` and `BuildError`.
* `LhaDecodeReader::seek_next_file()` added, using `io::Seek` instead of discarding read data.
* `LhaDecodeReader::into_parts()`, `get_decoder()`, `get_mut_decoder()` and `take_decoder()` added.
* `get_ref()` and `get_mut()` methods added allowing access to the underlying reader stream.
* `CompressionMethod::is_compressed()` added.
* `LhaHeader::parse_unix_uid_gid()` added.
* `LhaHeader::parse_unix_permissions()` and a new `Permissions` bitflag object added.
* `LhaHeader::parse_os_9_attrs()` and a new `Os9Attrs` bitflag object added.
* `LhaHeader` now derives `PartialEq` and `Eq`.
* `fmt::Display` implementation added to `MsDosAttrs` and `OsType`.
* `LHARK` OS ID recognized.

Fixes:
* Remove links to integration tests and examples form the manifest - the files were never included in the crate.
* Replace open-ended ranges with inclusive ones to be able to yield MAX values when iterating.
* `HuffTree`: ensure a tree is cleared on any error in the `build_tree()` function.
* `HuffTree` and `DynHuffTree`: removed `fmt::Display` implementation outside of testing.
* Fixed `LhaV2Decoder::begin_new_block()` to set `remaining_commands` only after successful tree decoding.
* `LhaV2Decoder`: reimplemented `read_temp_tree()` in such a way that if the `skip` value is too large,
ignore it, and let the tree building method handle it, instead of returning a specific error.


v0.7.0
* Rust edition: 2024.
* Minimum supported rust version changed to Rust 1.93 (slice::assume_init_mut).
* Deps: `bytemuck` added and `bitflags` bumped to 2.13.
* Replaced all uses of `mem::transmute()` with modern functions.
* clippy suggested changes.


v0.6.2
* Minimum supported rust version changed to Rust 1.65.
* Fixed a bug in `LhaHeader::read()` that could cause integer overflow.
* Fixed a bug in `LhaHeader::read()` that could allocate a huge memory chunk before failing.
* Added a static assert to prevent compilation on systems with `usize` < 32-bit.
* Added missing `long_header_len` check on lha_level=3 in in `LhaHeader::read()`.
* Fixed minor warnings.


v0.6.1
* Fixed a bug in `LhaV2Decoder::read_temp_tree()` that might cause a panic on a random bitstream.
* Deps: `bitflags` bumped to 2.5.


v0.6.0
* `no-std` is enabled in the absence of the `std` feature.
* Breaking changes for exported types and function signatures:
  - Generic types and methods depending on `std::io::Read` now depend on `stub_io::Read`.
  - Methods previously returning `std::io::Error` return `LhaError` instead.
  - `LhaHeader::read()` signature argument changed to &mut R.
* `LhaHeader::parse_pathname_to_string added()`.
* `LhaDecodeReader::next_file_with_sink()` added.
* `TimestampResult::to_local()` is only available with std feature.
* `LhaHeader::parse_pathname()` is only available with std feature.
* `delharc::parse_file()` is only available with std feature.
* Deps: `bitflags` bumped to 2.4.
* extract_nostd example added to showcase usage of nostd.
* Embedded example added to test compilation of a no-std target.


v0.5.0
* Rust edition: 2021.
* Minimum supported rust version changed to Rust 1.63 (array::from_fn).
* An example added to showcase usage of different reader sources.
* RingArrayBuf reworked with const generics.
* dyntree: unsafe transmute replaced with array::from_fn.
* Deps: `bitflags` updated to 2.3.
* License files added.


v0.4.0
* Minimum supported rust version changed to Rust 1.46 (const fn improvements).
* CI: migration to Github Actions.
* Tests: pass all tests regardless of selected features.
* Deps: `bitflags` updated to 1.3, dev-deps updated.


v0.3.0
* `LhaHeader::parse_comment()` added.
* `LhaHeader::parse_pathname()` returns the file name up to a `nul` character with Amiga archives.


v0.2.2
* Removed unnecessary static lifetime constraint on the inner reader of `LhaDecodeReader`.


v0.2.1
* Fixed result of `LhaHeader::is_supported()` and `LhaDecodeReader::is_decoder_supported()`.


v0.2.0
* Added a missing `fmt::Debug` trait implementation for `LhaDecodeReader`.
* Some methods of `LhaDecodeReader` now return an error variant as `LhaDecodeError`, so the stream source can be retrieved.
* Fixed a minor bug in the documentation.


v0.1.0
* The initial release.
