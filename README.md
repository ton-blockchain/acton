# Acton

<img align="right" src="docs/public/logo.png" height="150px" alt="Acton logo" />

Acton is an all-in-one TON smart contract development toolkit written in Rust.
It combines project scaffolding, build, testing, scripting, wallet and network
operations, verification, linting, formatting, and low-level VM tooling in one
CLI.

Documentation: https://i582.github.io/acton/docs/welcome

<br clear="right" />

## Why Acton

- Single CLI for the full contract lifecycle: create, build, test, deploy,
  verify.
- Native speed (Rust-based toolchain and test runtime).
- Tolk-first workflow with built-in wrappers, testing utilities, and scripts.
- Local development node with faucet, forking, snapshots, and persistence.

### Build from source

Acton links static TON artifacts (`libemulator.a`, `libtolk.a`) from the
`i582/ton` fork branch `pmakhnev/acton`.

```bash
# 1) clone repositories
git clone https://github.com/i582/acton.git
git clone --branch pmakhnev/acton https://github.com/i582/ton.git ton-repo --recurse-submodules

# 2) build TON static artifacts (example for Linux)
cd ton-repo
sh ./assembly/native/build-ubuntu-static.sh -a -c
# or sh ./assembly/native/build-macos-static.sh -a -c
cd ..

# 3) copy artifacts into Acton
mkdir -p acton/objs
cp ton-repo/artifacts/libemulator.a acton/objs/
cp ton-repo/artifacts/libtolk.a acton/objs/

# 4) build UI assets (required)
cd acton
just build-ui

# 5) build Acton
cargo build
./target/debug/acton --help
```

## Run

```
target/debug/acton test foo.test.tolk
# or target/release/acton test foo.test.tolk
```

## Development

See [CONTRIBUTING.md](CONTRIBUTING.md).

## License

MIT
