# Contributing to rston

Run these commands from `libs/rston` in an Acton checkout.
The library has its own Cargo workspace, which includes `rston-proc` and the fuzz targets.

## Tests

Run the tests and documentation examples with all features:

```sh
cargo test --workspace --all-features
```

Check the library without default features:

```sh
cargo check --lib --no-default-features
```

## Benchmarks

Run the BoC or dictionary benchmarks:

```sh
cargo bench --bench boc
cargo bench --bench dict
```

The [benches](benches/) directory contains additional targets for dictionary updates, cell slices, and cell usage tracking.
Each target has an entry in [Cargo.toml](Cargo.toml).
The `callgrind_*` targets require Valgrind with Callgrind.

## Formatting and Clippy

The formatter configuration requires nightly Rust.
Install nightly Rust with rustfmt:

```sh
rustup toolchain install nightly --profile minimal --component rustfmt
```

Format the workspace:

```sh
cargo +nightly fmt --all
```

Run the formatting and Clippy checks:

```sh
cargo +nightly fmt --all -- --check
cargo clippy --workspace --all-features -- -D warnings
```

## Miri

Install Miri for nightly Rust:

```sh
rustup +nightly component add miri
```

Run the library tests under Miri:

```sh
cargo +nightly miri test --lib
```

## Fuzzing

Install cargo-fuzz:

```sh
cargo install cargo-fuzz --locked
```

Run a BoC decoder target:

```sh
cargo +nightly fuzz run boc_decode
```

Run the BoC encode/decode target:

```sh
cargo +nightly fuzz run boc_decode_encode
```

The [fuzz/fuzz_targets](fuzz/fuzz_targets/) directory contains the other targets.
Use the target name from [fuzz/Cargo.toml](fuzz/Cargo.toml) with `cargo +nightly fuzz run`.

## API documentation

Build the API documentation:

```sh
cargo doc --no-deps --all-features --open
```
