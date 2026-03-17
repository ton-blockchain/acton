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

## Install

The recommended way to get Acton today is to download a prebuilt binary from
release `v0.0.16`:

| Platform | Architecture | Download                                                                                                                                 |
|----------|--------------|------------------------------------------------------------------------------------------------------------------------------------------|
| macOS    | ARM64        | [acton-aarch64-apple-darwin.tar.gz](https://github.com/i582/acton/releases/download/v0.0.16/acton-aarch64-apple-darwin.tar.gz)           |
| macOS    | x86_64       | [acton-x86_64-apple-darwin.tar.gz](https://github.com/i582/acton/releases/download/v0.0.16/acton-x86_64-apple-darwin.tar.gz)             |
| Linux    | x86_64       | [acton-x86_64-unknown-linux-gnu.tar.gz](https://github.com/i582/acton/releases/download/v0.0.16/acton-x86_64-unknown-linux-gnu.tar.gz)   |
| Linux    | ARM64        | [acton-aarch64-unknown-linux-gnu.tar.gz](https://github.com/i582/acton/releases/download/v0.0.16/acton-aarch64-unknown-linux-gnu.tar.gz) |

After extracting the archive, make sure `acton` is on your `PATH` and verify
the installation:

```bash
acton --version
```

For more installation details, see the
[installation guide](https://i582.github.io/acton/docs/installation).

## From zero to testnet

```bash
# Create a new project from the built-in counter template
acton new first_counter --template counter
cd first_counter

# Build and test locally
acton build
acton test

# Create and fund a local testnet wallet
acton wallet new --name deployer --local --airdrop

# Deploy to TON testnet
acton script scripts/deploy.tolk --broadcast --net testnet
```

For a step-by-step walkthrough, see the
[quickstart guide](https://i582.github.io/acton/docs/quickstart).

## Building from source

Source builds are intended for contributors and local development. See
[Building from source](CONTRIBUTING.md#building-from-source) in CONTRIBUTING.md.

## Contributing

Contributor setup, test workflows, UI build steps, and docs workflows are in
[CONTRIBUTING.md](CONTRIBUTING.md).

## License

Acton is licensed under either of

- Apache License, Version 2.0, ([LICENSE-APACHE](./LICENSE-APACHE) or https://www.apache.org/licenses/LICENSE-2.0)
- MIT license ([LICENSE-MIT](./LICENSE-MIT) or https://opensource.org/licenses/MIT)

at your option.

Unless you explicitly state otherwise, any contribution intentionally submitted for
inclusion in Acton by you, as defined in the Apache-2.0 license, shall be dually licensed
as above, without any additional terms or conditions.
