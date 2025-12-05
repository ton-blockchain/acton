<img width="150px" src="docs/img/logo.png">

# Acton

Blazingly fast ~~shit~~ toolkit for TON application development written in
Rust.

## Building

Clone TON monorepo fork:

```
git clone https://github.com/i582/ton/tree/pmakhnev/acton
```

Build and copy artifacts to `./objs`:

```
sh assembly/native/build-macos-static.sh -a && ../acton/objs && cp ./artifacts/libemulator.a ./artifacts/libtolk.a ../acton/objs
```

Run Rust compilation:

```
cargo build --bin acton
```

In release mode:

```
cargo build --bin acton --release
```

## Run

```
target/debug/acton test foo_test.tolk
# or target/release/acton test foo_test.tolk
```

## Testing

See [Documentation](https://i582.github.io/acton/test-runner/your-first-unit-test-in-tolk/).

## Scripts

See [Documentation](https://i582.github.io/acton/scripting/overview/).

## Compilation

See [Documentation](https://i582.github.io/acton/compile/).

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
