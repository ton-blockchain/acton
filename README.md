<img width="150px" src="docs/public/logo.png">

# Acton

Blazingly fast ~~shit~~ toolkit for TON application development written in
Rust.

## Building

Ensure the TON C++ sources are available in `./ton-acton` and build prerequisites
are installed (cmake, ninja, a C++ toolchain, llvm-objcopy/objcopy, plus system
libs like libsodium/openssl/zlib). The build uses the same requirements as the
native scripts in `ton-acton/assembly/native`.

Run Rust compilation (this also builds the required C++ libraries). Debug builds
use `CMAKE_BUILD_TYPE=Debug` so native objects include debug symbols.

```
cargo build
```

In release mode:

```
cargo build --release
```

To force a rebuild of the native libraries:

```
ACTON_NATIVE_FORCE_REBUILD=1 cargo build
```

## Run

```
target/debug/acton test foo.test.tolk
# or target/release/acton test foo.test.tolk
```

## Documentation

See [Documentation](https://i582.github.io/acton/docs/welcome/).

## Development

Run all tests:

```
just test
```

Run integration tests:

```
cargo test --test integration_test
```

Run debugger tests:

```
cargo test --test debug_test -- --test-threads 1
```

To update snapshots set `SNAPSHOTS=overwrite`.

See also: [justfile](justfile) for all commands.
