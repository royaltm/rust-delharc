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
