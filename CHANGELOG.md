# Changelog

All notable changes to this project will be documented in this file.

## [Unreleased]

### Added

- No unreleased entries yet.

## [0.1.4] - 29.03.2026

Acton 0.1.4 adds remote account inspection and deeper test-runner control,
while improving wallet, broadcast, and diagnostics workflows around real-network
interactions.

### Added

- Added `acton rpc info` to inspect a remote account, print status and hash metadata, match local contracts by `code_hash`, and decode on-chain storage through local ABI metadata when available.
- Added iterative test-runner execution via `net.sendIter()` and `TxCursor` for partial transaction-chain execution, targeted stopping, and interleaving multiple message cascades against shared emulated state.
- Added world-state snapshot APIs `net.saveSnapshot()` and `net.loadSnapshot()` to persist local emulator state as JSON fixtures and restore it in later test runs.

### Changed

- Changed interactive testnet airdrops in `acton wallet new` and `acton wallet airdrop` to wait briefly for balance confirmation by default, with `--no-wait-airdrop` available to skip the wait.
- Changed broadcast-mode transaction waiting defaults to poll more frequently after submission, reducing unnecessary delay once the message is already on the network.
- Improved real-network send diagnostics across wallet-driven workflows to explain common failures such as missing wallet state, insufficient balance, stale seqno, expired messages, and the right `acton wallet airdrop` fix for testnet or localnet.
- Improved wallet creation and import output with a clearer follow-up hint for checking balances via `acton wallet list --balance`.

### Fixed

- Fixed `acton doctor` API reachability checks to degrade more gracefully in restricted sandboxed environments instead of failing with opaque proxy-discovery panics.
- Fixed test-runner `isContractDeployed()` detection for missing and explicit-null account states.

## [0.1.3] - 28.03.2026

Acton 0.1.3 expands built-in command manuals and diagnostics, improves Linux
binary compatibility on older CPUs, refreshes TON objs metadata, and hardens
CLI workflows with broader integration coverage.

### Added

- Added long-form built-in manuals for top-level commands via `acton help <command>`.
- Added bundled plain-text manual artifacts generated from the CLI definitions.
- Added bundled manpage artifacts generated from the CLI definitions.
- Added API reachability checks to `acton doctor` for common Acton backends.
- Added native `.a` library version reporting to `acton doctor`.
- Added embedded TON commit hash and date reporting for native libraries in `acton doctor`.

### Changed

- Changed Linux TON objs builds to avoid `-march=native`, improving compatibility on older CPUs.
- Refreshed the checked-in TON objs artifact manifest to the latest upstream snapshot.

### Fixed

- Fixed incorrect flag references in the quickstart and test-runner documentation.

## [0.1.2] - 27.03.2026

Acton 0.1.2 improves project scaffolding, script diagnostics, and secure wallet storage,
while tightening release maintenance workflows and TON objs artifact validation.

### Added

- Added `acton new --agents` to include template-specific `AGENTS.md` guidance for coding agents, with matching interactive prompts in project creation flows.
- Added richer `acton script` failure diagnostics, including exit code descriptions and phases, plus source backtraces when re-run with `--backtrace full`.

### Changed

- Changed secure wallet storage to keep per-scope mnemonic bundles in the native keychain, so multiple secure wallets in one local or global scope can share a single keychain entry and be updated or removed independently.
- Changed TON objs archive validation to use the checked-in `artifacts_manifest.toml`, with `TON_OBJS_DISABLE_ARCHIVE_SHA_VERIFY=1` available as an escape hatch for local archive refresh workflows.

### Fixed

- Fixed the `jetton` starter template for the latest Tolk 1.3 syntax and corrected its generated `deploy.tolk` script.
- Fixed a dependency security issue by bumping `tar` to `0.4.45`.

### Upgrade Notes

- If you use secure wallets backed by the native keychain, re-import or recreate them after upgrading so Acton can rewrite the stored mnemonic data in the new bundled format.
- If your CI uses the GitHub Action, update workflow references to `ton-blockchain/setup-acton`.

## [0.1.1] - 22.03.2026

Added TON objs files to releases.

## [0.1.0] - 22.03.2026

Acton 0.1.0 is the first semi-stable beta release with a complete installation and delivery story.
It makes the CLI easy to install from official artifacts while keeping the project on the beta release channel.

### Added

- Added an official shell installer (`acton-installer.sh`) for public beta releases.
- Added official release artifacts for four first-class platforms: macOS (ARM64, x86_64) and Linux GNU (ARM64, x86_64).
- Added a full CI and release pipeline around builds, release validation, artifact checks, public release mirroring, and dependency and security checks.
- Added broader developer tooling over the last few months, including `acton doctor`, `func2tolk`, `Acton.toml` schema generation, better starter templates, and improved TypeScript wrapper generation.

### Changed

- Promoted Acton to the `0.1.x` beta release line.
- Changed the recommended installation flow to use the public installer and official release artifacts.
- Improved `acton up`, templates, wrappers, localnet and network workflows, test reporting, and documentation across the project.

### Fixed

- Fixed numerous issues across CI, release automation, tests, documentation, wrappers, wallets, localnet and litenode integration, formatter output, and diagnostics.
- Fixed multiple flaky tests and platform-specific issues, especially around macOS and release workflows.
- Fixed many smaller bugs and polish issues accumulated over the last few months across the CLI, compiler-facing tooling, and project templates.

### Upgrade Notes

- Prefer installing or updating via `acton-installer.sh` or the official release archives.
- First-class public artifacts are available for macOS (ARM64, x86_64) and Linux GNU (ARM64, x86_64).
- If you use generated TypeScript wrappers, note that recent releases now emit them into `wrappers/` by default.

## [0.0.21] - 21.03.2026

### Added

- Added a `counter` starter template with a React + Vite app for `acton new`.
- Added `func2tolk --version`.

### Changed

- Changed generated TypeScript wrappers to go to `wrappers/` by default.

### Upgrade Notes

- If you rely on generated TypeScript wrappers, update any tooling that expected the previous default output location.
- Project references now use the `ton-blockchain/acton` repository path.

### Internal

- Added `cargo xtask schema` to generate the `Acton.toml` JSON schema.
- Added baseline maintainer and project docs, including release, support, security, and conduct policies.
- Improved CI and release automation reliability across release checks and macOS workflows.

## [0.0.20] - 18.03.2026

First version with completed CI.
