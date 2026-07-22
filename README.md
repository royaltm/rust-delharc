delharc
=======

[![Crate][Crate img]][Crate Link]
[![Docs][Docs img]][Docs Link]
[![Build Status][Build img]][Build Link]
[![Coverage Status][Coverage img]][Coverage Link]
[![Minimum rustc version][rustc version img]][rustc version link]

A [Rust] library for parsing and extracting content of [LHA/LZH] archives.

What it does
------------

This library provides ways to parse the content of **LHA** headers and allows to read files, compressed with the methods used by the archive format.

Files using this format usually have `.lzh` or `.lha` extensions, or `.pma` in case of the CP/M archiver. Some formats, such as the retro chip-tune [YM] format, use **LHA** as its default packaging method. The entire content of the [Aminet] has also been packaged using this format.


What it doesn't do
------------------

This library does not provide high level methods for creating files or directories from the extracted archives.


Usage
-----

Add to `Cargo.toml`:

```toml
[dependencies]
delharc = "0.8"
```

For more information, please see the [Documentation][Docs Link].


No std
------

Since version 0.6, `delharc` can be used without the `std` library. In this instance the `alloc`
external crate will be required instead.

```toml
[dependencies.delharc]
version = "0.8"
default-features = false
features = ["lh1", "lz", "pm"] # select desired features
```

`delharc` API was originally built around the `std::io` types such as `io::Error` and `io::Read`.

This design choice made it impossible to adapt `delharc` to be used in the absence of the `std::io`
library without some significant implementation changes.

To work around this problem the `stub_io` module and `error` module was added. `stub_io`
contains an I/O proxy trait `Read` and a `Take` type which are now used as interfaces for generic
types throughout the library. Instead of relying on `io::Error` for fallible results `delharc`
defines its own `error::LhaError` which encapsulates an I/O error type.

With `std` library enabled, `error::LhaError` converts to `io::Error` via the `From` trait and
`stub_io::Read` is implemented for all `io::Read` implementations.

For `std` users the difference from previous versions is that methods previously returning
`io::Result` now return `Result<_, LhaError<io::Error>>`. This might break cases when result
`Err(error)` from calls to `delharc` methods is returned as is without the `?` or `From` conversion.

Now, when using `default-features = false` the `std` feature needs to be added back.


Upgrading
---------

A lot has happened in version `0.8`, including some **breaking** changes. Please consult the [CHANGELOG](CHANGELOG.md) for the full list of updates.


Rust Version
------------

`delharc` requires Rustc version 1.95 or greater.


Acknowledgements
----------------

The decompression functions in this library, with minor modifications, were implemented in Rust by the author of this project, based on [LHASA - Free Software LHA implementation](https://fragglet.github.io/lhasa/) written in C.

Many thanks to [Simon Howard] for collecting and reverse engineering all these algorithms into one very good and understandable source code.


License
-------

This project is licensed under either of

 * MIT license ([LICENSE-MIT](LICENSE-MIT) or
   https://opensource.org/licenses/MIT)
 * Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or
   https://www.apache.org/licenses/LICENSE-2.0)

at your option.

[Rust]: https://www.rust-lang.org/
[LHA/LZH]: https://en.wikipedia.org/wiki/LHA_(file_format)
[Aminet]: https://aminet.net/
[YM]: http://leonard.oxg.free.fr/ymformat.html
[Crate Link]: https://crates.io/crates/delharc
[Crate img]: https://img.shields.io/crates/v/delharc.svg
[Docs Link]: https://docs.rs/delharc
[Docs img]: https://docs.rs/delharc/badge.svg
[Build Link]: https://github.com/royaltm/rust-delharc/actions/workflows/ci.yml
[Build img]: https://github.com/royaltm/rust-delharc/actions/workflows/ci.yml/badge.svg?branch=master
[Coverage Link]: https://coveralls.io/github/royaltm/rust-delharc?branch=master
[Coverage img]: https://coveralls.io/repos/github/royaltm/rust-delharc/badge.svg?branch=master
[rustc version link]: https://github.com/royaltm/rust-delharc#rust-version
[rustc version img]: https://img.shields.io/badge/rustc-1.95+-lightgray.svg
[Simon Howard]: https://github.com/fragglet
