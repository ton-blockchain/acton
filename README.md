# Acton

<img align="right" src="docs/public/logo-light.svg" height="150px" alt="Acton logo" />

Acton is an end-to-end platform for building TON dApps. It guides projects from
the first smart contract to a live application and supports every step along
the way.

Built for **humans**. Perfect for **AI**.

### [Read the docs →](https://ton-blockchain.github.io/acton/docs/welcome)

<br clear="right" />

## Act on-chain: one platform for every step

- **Start a project.** Scaffold a new application or bring an existing one
  under a single `Acton.toml` project model for contracts, dependencies,
  networks, scripts, tests, and generated artifacts.
- **Write contracts.** Work in
  [VS Code-based editors](https://marketplace.visualstudio.com/items?itemName=ton-core.vscode-ton)
  or [JetBrains IDEs](https://plugins.jetbrains.com/plugin/23382-ton) with
  first-class Acton integration, backed by the bundled Tolk compiler and
  standard libraries, formatter, linter, and a built-in language server for
  Tolk, TL-B, and Fift/TASM.
- **Build contracts.** Dependency-aware compilation, caching, generated Tolk
  wrappers, and BoC-backed artifacts keep multi-contract projects fast and
  reproducible.
- **Test behavior.** Write tests in Tolk and execute complete transaction chains
  locally. Cover internal and external message flows, reproduce scenarios from
  pinned Mainnet or Testnet state, and strengthen the suite with fuzzing,
  mutation testing, and coverage.
- **Optimize costs.** After tests pass, use gas snapshots, per-function reports,
  and the `bench` module to find expensive code and measure the impact of
  optimizations.
- **Debug failures.** Open a failed test in the browser Test UI, or retrace an
  on-chain transaction locally. Source maps connect contract code to VM logs,
  storage changes, actions, and value flow.
- **Build the dApp.** Once the contracts are tested, generate TypeScript
  wrappers and connect them to the client application.
- **Run locally.** Start `acton localnet` to run the contracts and client
  together on a fast simulated TON network, with an optional fork when you need
  state from a public network. You keep full control over network state and
  block production, and can run the same environment in CI.
- **Move beyond simulation.** When `acton localnet` is not enough, use Localton
  to start real TON nodes and the supporting network stack on your computer,
  then test the dApp locally before moving to Testnet.
- **Work visually.** Acton Studio brings test history, local environments,
  explorer views, wallets, and message simulation into one browser workspace
  for developing and debugging the entire dApp.
- **Deploy to Testnet.** Run the same scripts you tested locally and fund the
  deployment wallet with
  [`acton wallet airdrop`](https://ton-blockchain.github.io/acton/docs/wallets/#fund-a-wallet-on-testnet)
  or use the [Actonscan faucet](https://testnet.actonscan.com/faucet).
- **Operate on-chain.** Wallet management, TON Connect approvals, on-chain
  libraries, custom networks, and explorer integration keep ongoing network
  operations in the same toolchain.
- **Verify and understand on-chain activity.** The
  [Acton Verifier](https://actonscan.com/verified?network=mainnet) publishes and
  checks contract source, the indexer processes network data, and
  [Actonscan](https://actonscan.com/) makes activity easy to follow on any
  network, including local environments.
- **Automate delivery.** CI-ready JSON, SARIF, GitHub, GitLab, and JUnit output,
  saved traces, and Docker workflows make the same development loop usable in
  automation.

## Install

The recommended way to get Acton today is to run the latest public installer:

```bash
curl -LsSf https://github.com/ton-blockchain/acton/releases/latest/download/acton-installer.sh | sh
```

If you prefer a manual download, use the latest public release:

| Platform | Architecture | Download                                                                                                                                          |
|----------|--------------|---------------------------------------------------------------------------------------------------------------------------------------------------|
| macOS    | ARM64        | [acton-aarch64-apple-darwin.tar.gz](https://github.com/ton-blockchain/acton/releases/latest/download/acton-aarch64-apple-darwin.tar.gz)           |
| macOS    | x86_64       | [acton-x86_64-apple-darwin.tar.gz](https://github.com/ton-blockchain/acton/releases/latest/download/acton-x86_64-apple-darwin.tar.gz)             |
| Linux    | x86_64       | [acton-x86_64-unknown-linux-gnu.tar.gz](https://github.com/ton-blockchain/acton/releases/latest/download/acton-x86_64-unknown-linux-gnu.tar.gz)   |
| Linux    | ARM64        | [acton-aarch64-unknown-linux-gnu.tar.gz](https://github.com/ton-blockchain/acton/releases/latest/download/acton-aarch64-unknown-linux-gnu.tar.gz) |

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

Acton is stable on the latest numbered GitHub release. The first-class platform
matrix is macOS (ARM64, x86_64) plus Linux GNU (x86_64, ARM64). For Linux, the
documented baseline is Ubuntu 20.04 or newer. Native Windows is not supported
today. If you use Windows, run Acton inside WSL with Ubuntu 20.04 or newer and
follow the Linux installation path there. `trunk` builds installed via
`acton up --trunk`, WSL installs, and other source-built targets are beta /
best-effort surfaces for now. The full policy is documented at
[Support policy](https://ton-blockchain.github.io/acton/docs/installation#support-policy).

## From zero to testnet

```bash
# Create a new project from the built-in counter template
acton new first_counter --template counter
cd first_counter

# Build and test locally
acton build
acton test

# Create and fund a locally stored testnet wallet
acton wallet new --name deployer --local --airdrop --version v5r1

# Deploy to TON testnet
acton script scripts/deploy.tolk --net testnet
```

For a step-by-step walkthrough, see the
[quickstart guide](https://ton-blockchain.github.io/acton/docs/quickstart).

Already have a repository instead of starting from a template? The existing
project path is:

```bash
cd your-repo
acton init
acton build
acton test
```

For more details, see the [Project management guide](https://ton-blockchain.github.io/acton/docs/projects).

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
