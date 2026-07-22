name := 'delharc'

embedded:
    cargo build -p delharc-embedded-example --target thumbv7em-none-eabihf

example:
    cargo build --examples
    cargo run --example extract
    cargo build --examples --no-default-features
    cargo run --example extract --no-default-features

# build all docs
doc:
    RUSTDOCFLAGS="--cfg docsrs" cargo +nightly doc --all-features

# run all normal tests
test:
    cargo build --no-default-features
    cargo test --no-default-features
    cargo build --no-default-features --features=lz,lh1,pm,lhx,fast-tree-build
    cargo test --no-default-features --features=lz,lh1,pm,lhx,fast-tree-build
    cargo build --no-default-features --features=lz,lh1,pm,lhx,extend,fast-tree-build
    cargo test --no-default-features --features=lz,lh1,pm,lhx,extend,fast-tree-build
    cargo build --no-default-features --features=std
    cargo test --no-default-features --features=std
    cargo build --no-default-features --features=std,lz
    cargo test --no-default-features --features=std,lz
    cargo build --no-default-features --features=std,lh1
    cargo test --no-default-features --features=std,lh1
    cargo build --no-default-features --features=std,pm
    cargo test --no-default-features --features=std,pm
    cargo build --features=std,lz,lh1,pm,lhx,extend,fast-tree-build
    cargo test --features=std,lz,lh1,pm,lhx,extend,fast-tree-build
    cargo build -r --features=std,lz,lh1,pm,lhx,extend,fast-tree-build
    cargo test -r --features=std,lz,lh1,pm,lhx,extend,fast-tree-build
    cargo build --all-features
    cargo test --all-features
    cargo build -r --all-features
    cargo test -r --all-features

# run all long tests
test-long:
    cargo test --no-default-features --features=std,lh1,lz,pm,extend -- --nocapture --ignored
    cargo test --no-default-features --features=std,lh1,lz,pm,extend,fast-tree-build -- --nocapture --ignored
    TEST_LONG_FILES=1 cargo test -r --no-default-features --features=std,lh1,lz,pm,extend,fast-tree-build -- --nocapture --ignored

# run clippy tests
clippy:
    touch src/lib.rs
    cargo clippy -- -D warnings
    cargo clippy --all-features -- -D warnings
    cargo clippy --no-default-features -- -D warnings
    cargo clippy --no-default-features --features=lh1,lz,pm,extend -- -D warnings
    cargo clippy --no-default-features --features=lh1,lz,pm,extend,fast-tree-build -- -D warnings
    cargo clippy --no-default-features --features=lh1,lz,pm,extend,fast-tree-build,no-unsafe-assertions -- -D warnings

clean:
    cargo clean
