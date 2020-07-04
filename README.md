delharc
=======

[![Crate][Crate img]][Crate Link]
[![Docs][Docs img]][Docs Link]
[![Build Status][Build img]][Build Link]
[![Coverage Status][Coverage img]][Coverage Link]
[![Minimum rustc version][rustc version img]][rustc version link]

A [Rust] library for parsing and extracting files from [LHA/LZH] archives.

Cargo.toml:

```toml
[dependencies]
delharc = "0.1"
```

For more information, please see the [Documentation][Docs Link].

Rust Version Requirements
-------------------------

`delharc` requires Rustc version 1.36 or greater due to the usage of some macro features and api that was introduced
or stabilized in this version.


[Rust]: https://www.rust-lang.org/
[LHA/LZH]: https://en.wikipedia.org/wiki/LHA_(file_format)
[Crate Link]: https://crates.io/crates/rust-delharc
[Crate img]: https://img.shields.io/crates/v/delharc.svg
[Docs Link]: https://docs.rs/delharc
[Docs img]: https://docs.rs/delharc/badge.svg
[Build Link]: https://travis-ci.org/royaltm/rust-delharc
[Build img]: https://travis-ci.org/royaltm/rust-delharc.svg?branch=master
[Coverage Link]: https://coveralls.io/github/royaltm/rust-delharc?branch=master
[Coverage img]: https://coveralls.io/repos/github/royaltm/rust-delharc/badge.svg?branch=master
[rustc version link]: https://github.com/royaltm/rust-delharc#rust-version-requirements
[rustc version img]: https://img.shields.io/badge/rustc-1.36+-lightgray.svg
