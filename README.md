# Acton

<img align="right" src="docs/public/logo.png" height="150px" alt="Acton logo" />

Acton is an all-in-one TON smart contract development toolkit written in Rust.
It combines project scaffolding, build, testing, scripting, wallet and network
operations, verification, linting, formatting, debugging, and low-level VM
tooling in one CLI.

Documentation: https://ton-blockchain.github.io/acton/docs/welcome

<br clear="right" />

## Why Acton

- Single CLI for the full contract lifecycle: create, build, test, debug,
  deploy, verify.
- Native speed (Rust-based toolchain and test runtime).
- Tolk-first workflow with built-in wrappers, testing utilities, and scripts.
- Browser test UI for failed tests, traces, logs, and coverage inspection.
- Local development node with faucet, forking, snapshots, and persistence.

## Install

The recommended way to get Acton today is to run the latest public installer:

```bash
curl --proto '=https' --tlsv1.2 -LsSf https://github.com/i582/acton-public/releases/latest/download/acton-installer.sh | sh
```

If you prefer a manual download, use the latest public release:

| Platform | Architecture | Download                                                                                                                                       |
|----------|--------------|------------------------------------------------------------------------------------------------------------------------------------------------|
| macOS    | ARM64        | [acton-aarch64-apple-darwin.tar.gz](https://github.com/i582/acton-public/releases/latest/download/acton-aarch64-apple-darwin.tar.gz)           |
| macOS    | x86_64       | [acton-x86_64-apple-darwin.tar.gz](https://github.com/i582/acton-public/releases/latest/download/acton-x86_64-apple-darwin.tar.gz)             |
| Linux    | x86_64       | [acton-x86_64-unknown-linux-gnu.tar.gz](https://github.com/i582/acton-public/releases/latest/download/acton-x86_64-unknown-linux-gnu.tar.gz)   |
| Linux    | ARM64        | [acton-aarch64-unknown-linux-gnu.tar.gz](https://github.com/i582/acton-public/releases/latest/download/acton-aarch64-unknown-linux-gnu.tar.gz) |

After extracting the archive, make sure `acton` is on your `PATH` and verify
the installation:

```bash
acton --version
```

If you prefer a containerized workflow, use the published Docker image:

```bash
docker run --rm ghcr.io/ton-blockchain/acton:<version> --version
```

To run Acton against the current project from Docker:

```bash
docker run --rm \
  -v "$PWD":/workspace \
  -w /workspace \
  ghcr.io/ton-blockchain/acton:<version> \
  build
```

For more installation details, see the
[installation guide](https://ton-blockchain.github.io/acton/docs/installation).

## Support policy

Acton is currently distributed as a public beta. The stable release channel is
the latest numbered GitHub release, and the first-class platform matrix is
macOS (ARM64, x86_64) plus Linux GNU (x86_64, ARM64). `trunk` builds installed
via `acton up --trunk`, native Windows usage, and other source-built targets
are beta / best-effort surfaces for now. The full policy is documented at
[Support policy](https://ton-blockchain.github.io/acton/docs/miscellaneous/support-policy).

## From zero to testnet

```bash
# Create a new project from the built-in counter template
acton new first_counter --template counter
cd first_counter

# Build and test locally
acton build
acton test

# Create and fund a local testnet wallet
acton wallet new --name deployer --local --airdrop --version v5r1

# Deploy to TON testnet
acton script scripts/deploy.tolk --broadcast --net testnet
```

For a step-by-step walkthrough, see the
[quickstart guide](https://ton-blockchain.github.io/acton/docs/quickstart).

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
