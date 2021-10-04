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

This library provides ways to parse the content of **LHA** headers and allows to read files, compressed with some of the methods used by the archive format.

Files using this format usually have `.lzh` or `.lha` extensions. Some formats, such as the retro chip-tune [YM] format, use **LHA** as its default packaging method. The entire content of the [Aminet] has also been packaged using this format.


What it doesn't do
------------------

This library does not provide high level methods for creating files or directories from the extracted archives.


Usage
-----

Add to `Cargo.toml`:

```toml
[dependencies]
delharc = "0.3"
```

For more information, please see the [Documentation][Docs Link].


Rust Version
------------

`delharc` requires Rustc version 1.46 or greater due to the newly allowed const fn expressions in this version.


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
[rustc version img]: https://img.shields.io/badge/rustc-1.46+-lightgray.svg
