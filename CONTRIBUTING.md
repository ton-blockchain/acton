# Contributing to Acton

Thanks for contributing to Acton.

Acton core is a Rust project, but the repository also includes UI components
that are built and checked with Bun.

This guide covers local setup, build steps, and contributor workflows used in
this repository.

## Useful references

- Public project overview: [README.md](README.md)
- Security reporting policy: [SECURITY.md](SECURITY.md)
- Maintainer release process: [RELEASING.md](RELEASING.md)

## Workspace map

This repository is a Cargo workspace for the CLI and Rust libraries, plus a few
non-Cargo surfaces such as docs and UI packages.

- Root `acton` crate (`src/`, root `Cargo.toml`): the CLI entrypoint and most
  end-user commands.
- Tolk language/compiler stack: `tolkc`, `tolk-*`, `tree-sitter-*`, and the
  matching `*-syntax` crates.
- Native/runtime bridge: `ton-objs`, `ton-executor`, `ton-emulator`, and
  `tvmffi`.
- Services and tooling: `ton-api`, `ton-localnet`, `ton-indexer`, `retrace`,
  `ton-ls`, and `acton-debug`.
- Repo tooling: `xtask` for release/schema/artifact maintenance workflows.
- Non-Cargo surfaces: `docs/` (Next.js + Fumadocs), the Bun-built UI crates,
  and template/package-manager assets under `src/commands/new/templates/`.

The `tree-sitter-*` crates own grammar source of truth. The matching
`*-syntax` crates are typed AST/parser wrappers around those grammars rather
than independent grammar implementations.

## Command conventions

- Unless stated otherwise, run commands from the repository root.
- Prefer `just` targets over ad-hoc commands when both exist.
- Some targets intentionally modify files (`just check-ui`, `just fmt-ui`, `just test-update`).

## Prerequisites

For the minimal local build/test flow, install:

1. Rust toolchain (`rustup`, `cargo`)
2. `just` (task runner)
   ```bash
   cargo install just --locked
   ```
3. `cargo-nextest` (required Rust test runner for non-doc tests)
   ```bash
   cargo install cargo-nextest --locked
   ```
4. Bun (required for UI packages)
   ```bash
   curl -fsSL https://bun.sh/install | bash
   ```
5. GitHub CLI (`gh`) (used by `just sync-artifacts`)

- macOS:
  ```bash
  brew install gh
  ```
- Linux (Debian/Ubuntu):
  ```bash
  sudo apt install gh
  ```

Optional CLI tools:

- `cargo-shear` (unused dependency linter for `just check-deps`, also needed by `just check` / `just check-ci`)
  ```bash
  cargo install cargo-shear --locked
  ```
- `cargo-deny` (dependency policy checks for `just check`, also used by `just check-security`)
  ```bash
  cargo install cargo-deny --locked
  ```
- `typos-cli` (spell checker for `just typos`, also needed by `just check` / `just check-ci`)
  ```bash
  cargo install typos-cli --locked
  ```
- `cargo-llvm-cov` (optional, for coverage)
  ```bash
  cargo install cargo-llvm-cov --locked
  rustup component add llvm-tools-preview
  ```

System dependencies:

- macOS:
  ```bash
  brew install libsodium libmicrohttpd pkg-config
  ```
- Linux (Debian/Ubuntu):
  ```bash
  sudo apt install libsodium-dev libmicrohttpd-dev pkg-config
  ```

For first-time Linux TON artifact builds (closer to CI), install the extended
toolchain set:

```bash
sudo apt-get update
sudo apt-get install -y \
  build-essential cmake ninja-build pkg-config autoconf automake \
  libssl-dev libtool zlib1g-dev libsecp256k1-dev libmicrohttpd-dev \
  libsodium-dev liblz4-dev libjemalloc-dev ccache \
  clang-16 llvm-16 libc++-16-dev libc++abi-16-dev
```

## Start working locally

For a fresh checkout, the shortest path to a working contributor setup is:

```bash
just sync-artifacts
just build-ui
just build-dev
./target/debug/acton --help
```

What this does:

1. `just sync-artifacts` syncs `crates/ton-objs/artifacts_manifest.toml`
   from the `release-objs` release and refreshes the matching prebuilt
   `objs/` archive plus bundled stdlib assets for your current platform.
2. `just build-ui` installs UI dependencies and builds the bundled UI assets.
3. `just build-dev` builds the debug CLI against the synced TON archives.
4. `./target/debug/acton --help` confirms that the binary starts.

## Building from source

Acton links static TON artifacts (`libemulator.a`, `libtolk.a`) from the
`i582/ton` fork branch `pmakhnev/acton`.

### Artifact ownership and verification

For normal contributor setup, treat the published `release-objs` assets as the
source of truth.

- `just sync-artifacts` / `cargo xtask sync-artifacts` owns
  `crates/ton-objs/artifacts_manifest.toml`, `objs/`, and the bundled stdlib
  assets under `crates/tolkc/assets/`.
- `crates/ton-objs/build.rs` verifies `libemulator.a` and `libtolk.a` against
  the SHA-256 values recorded in that manifest.
- Only use the manual rebuild path when you are intentionally updating the
  native artifact set itself; otherwise prefer re-syncing from `release-objs`.

The verification bypass `TON_OBJS_DISABLE_ARCHIVE_SHA_VERIFY` exists as an
escape hatch, but it should stay unset for normal contributor builds. When set
to anything other than `0` / `false`, build-time archive verification is
disabled.

### Option 1: sync prebuilt `objs` with xtask

Use the built-in sync task instead of downloading release assets manually:

```bash
just sync-artifacts
just build-ui
just build-dev
./target/debug/acton --help
```

This command:

- downloads the current `artifacts_manifest.toml` from the `release-objs` release;
- overwrites `crates/ton-objs/artifacts_manifest.toml` with the released manifest;
- downloads and unpacks the matching `ton-objs-<target>.tar.gz` archive into
  `objs/`;
- downloads a temporary `ton-stdlib.tar.gz`, replaces
  `crates/tolkc/assets/tolk-stdlib/` and `crates/tolkc/assets/fift-stdlib/` from its
  `tolk-stdlib/` directory and `fift-stdlib/Asm.fif` plus
  `fift-stdlib/Fift.fif`, then removes
  the temporary archive.

After syncing, validate with:

```bash
just build-dev
./target/debug/acton doctor
```

### Option 2: build TON artifacts manually

Clone the TON repository from the Acton repo root:

```bash
git clone --branch pmakhnev/acton https://github.com/i582/ton.git ton-repo --recurse-submodules
```

Build the static artifacts with the script for your platform.

Ubuntu/Debian-like Linux:

```bash
cd ton-repo
sh ./assembly/native/build-ubuntu-static.sh -a -c
cd ..
```

macOS:

```bash
cd ton-repo
sh ./assembly/native/build-macos-static.sh -a -c
cd ..
```

Then copy the generated archives into Acton and build the project:

```bash
mkdir -p objs
cp ton-repo/artifacts/libemulator.a objs/
cp ton-repo/artifacts/libtolk.a objs/
# only when intentionally updating the native artifact set, edit crates/ton-objs/artifacts_manifest.toml:
# - increment `artifact_set_revision`
# - update `sha256.libemulator` / `sha256.libtolk`

just build-ui

just build-dev
./target/debug/acton --help
```

## Running Acton locally

```bash
./target/debug/acton --help
./target/debug/acton test
```

## Test Workflows

Default test command:

```bash
just test
```

Update snapshots:

```bash
just test-update
```

Equivalent explicit form (with env):

```bash
SNAPSHOTS=overwrite just test
```

Run specific suites:

```bash
# Integration tests
cargo nextest run --test integration_test

# Debugger tests (sequential due to shared debug port)
cargo nextest run --test debug_test --test-threads 1
```

Keep temp test artifacts:

```bash
DISABLE_TMP_DIR_CLEANUP_IN_TESTS=1 just test
```

Notes:

- `just test` uses `cargo nextest run` for Rust test targets and
  `cargo test --workspace --doc` for doctests.
- `retrace` tests stay inside the shared workspace run, but the repo's nextest
  config assigns `package(retrace)` to the `retrace-serial` group, so those
  tests still run one at a time.
- CI test behavior is slightly different: the `ci` nextest profile and
  `only_ci` feature are enabled in CI.
  For parity, run:
  ```bash
  CI=1 just test
  ```

## Formatting and Linting

Rust:

```bash
just fmt
just fmt-check
just clippy
just check-deps
just typos
```

UI:

```bash
just fmt-ui
just check-ui
```

`just check-ui-ci` matches the non-mutating UI check used in CI.
`just check-ui` runs `lint:fix` and may modify files.
Run it until no further changes are produced, then stage updated files.

`just typos` checks the repository from the root using `_typos.toml`.
It skips `docs/` and selected generated or imported trees with high false-positive rates.

## Security checks

Run repository-wide dependency and supply-chain audits with:

```bash
just check-security
```

This target stops on the first failure and currently checks:

- Rust dependencies with `cargo deny check`
- root/UI workspace dependencies with `bun audit`
- `docs/`, `crates/tree-sitter-*`, and `crates/ton-ls/editors/code` with
  `yarn npm audit`
- `src/commands/new/templates/counter-app` with `npm audit`

Run it when your PR changes lockfiles, dependency manifests, or package
versions anywhere in the repository.

## Coverage

Generate LCOV:

```bash
just coverage
```

Generate HTML report:

```bash
just coverage-html
```

## UI Build

Build UI bundles used by Acton:

```bash
just build-ui
```

## Documentation workflows

Documentation site (Next.js in `docs/`, package manager: Bun):

```bash
cd docs
bun install
bun run dev
```

Build docs:

```bash
cd docs
bun ci
bun run build
```

Regenerate auto-generated MDX documentation from Acton sources:

```bash
./target/debug/acton docgen
# or
cargo run --bin acton -- docgen
# check mode (fails with exit code 1 and prints diff if docs are stale)
cargo run --bin acton -- docgen --check
```

Do not hand-edit generated outputs. Edit the source-of-truth inputs instead,
then rerun `acton docgen`.

Source-of-truth map:

- `src/doc/man/*.md` -> command reference docs under `docs/content/docs/commands`,
  terminal help text under `src/doc/man/generated_txt`, and installed manpages
  under `src/etc/man`
- `lib/` -> `docs/content/docs/standard_library`
- `crates/tolkc/assets/tolk-stdlib/` -> `docs/content/docs/tolk_standard_library`
- linter rule metadata in `crates/tolk-linter/` and related macros ->
  `docs/content/docs/linting/rules`

This updates generated trees under:

- `docs/content/docs/commands`
- `docs/content/docs/standard_library`
- `docs/content/docs/tolk_standard_library`
- `docs/content/docs/linting/rules`
- `src/etc/man`
- `src/doc/man/generated_txt`

`acton docgen --check` renders into a temporary output tree and fails if any
tracked generated file is stale.

If your PR changes any docgen inputs, running `acton docgen` and committing
generated documentation changes is required. This includes:

- `lib/`
- `crates/tolkc/assets/tolk-stdlib/`
- linter rule metadata and mappings (for example `crates/tolk-linter/`,
  `crates/tolk-macros/`)

For docs-site-only pages under `docs/content/docs/` that are not generated,
edit them directly and keep nearby `meta.json` in sync.

After doc updates (manual or generated), validate docs build:

```bash
cd docs
bun ci
bun run build
```

## Schema workflow

`acton.schema.json` is generated, not hand-maintained.

Useful commands:

```bash
cargo xtask schema
cargo xtask schema --check
just check-schema
```

When to rerun schema generation:

- any change to `ActonConfig` or related config structs
- schema-shaping serde/schemars changes
- docs or editor work that depends on new config fields

Current consumers include:

- repo editor settings such as `.vscode/settings.json`
- `ton-ls`, which embeds the schema for TOML hover/completion help

A stale schema usually shows up as `just check-schema` failure, missing hover
docs, or editor completion that does not know about new config fields.

## Tree-sitter workflows

Grammar source of truth lives in `crates/tree-sitter-*`. The matching
`crates/*-syntax` crates wrap those grammars with typed AST helpers.

If your PR changes any `crates/tree-sitter-*` grammar/parser artifacts:

```bash
just test-tree-sitter-all
```

For quick Tolk-only iteration:

```bash
just test-tree-sitter-tolk
```

When grammar snapshots need refresh:

```bash
just update-test-tree-sitter-tolk
```

## `xtask` map

`cargo xtask` contains both contributor-facing and maintainer-only workflows.

Common contributor-facing tasks:

- `cargo xtask sync-artifacts`
- `cargo xtask schema`
- `cargo xtask schema --check`

Mostly maintainer-facing tasks:

- `cargo xtask release`
- `cargo xtask retag`
- `cargo xtask dist ...`
- `cargo xtask github-cleanup`
- `cargo xtask ubicloud-cleanup`

`RELEASING.md` documents numbered release flows in detail. The cleanup tasks are
cache-pruning helpers and should not be used casually.

## Change-based checklist

Use this as a quick local matrix before pushing:

| Change type                                                                                                               | Required local checks                                                                                     |
|---------------------------------------------------------------------------------------------------------------------------|-----------------------------------------------------------------------------------------------------------|
| Rust-only code                                                                                                            | `just check`                                                                                              |
| UI code (`crates/acton-*-ui`, root `package.json`)                                                                        | `just check` + `just build-ui` + `just check-ui`                                                          |
| Dependency or lockfile changes (`Cargo.lock`, `bun.lock`, `docs/`, tree-sitter/code extension/template package manifests) | `just check-security`                                                                                     |
| Standard library / docgen inputs (`lib/`, `crates/tolkc/assets/tolk-stdlib`, linter rule metadata)                        | `just check` + `acton docgen` and  commit generated docs                                                  |
| Docs site content/config (`docs/`)                                                                                        | `cd docs && bun ci && bun run build`                                                                       |
| Tree-sitter grammar (`crates/tree-sitter-*`)                                                                              | `just check` +`just test-tree-sitter-all` (and `just update-test-tree-sitter` when Tolk snapshots change) |
| Release preparation (maintainers)                                                                                         | Follow [RELEASING.md](RELEASING.md)                                                                       |

## PR requirements

Every pull request must pass all checks from:

```bash
just check
```

This command runs Rust formatting, docgen, dependency, dependency-policy, lint, schema, and test checks.
Install `cargo-shear`, `cargo-deny`, and `typos-cli` if you want to run it locally.
`typos` uses `_typos.toml` excludes for `docs/` and selected generated or imported trees.

If your PR touches UI code (`crates/acton-test-ui`, `crates/acton-localnet-ui`,
`crates/acton-shared-ui`, or root UI config in `package.json`), you must also
run:

```bash
just build-ui
just check-ui
```

Recommended extended local validation before opening a PR:

```bash
just precommit
```

`just precommit` additionally runs UI formatting/lint and full build steps.

## Commit message style

Project follows Conventional Commits with scope.

Recommended format:

```text
<type>(<scope>): <short imperative summary>
```

Examples:

- `feat(check): add inspection for duplicate condition in if-else`
- `fix(stdlib): fix external message body serialization`
- `chore(docs): update linting docs`

Rules:

- Use one of: `feat`, `fix`, `chore`, `refactor` (and `docs`/`test`/`ci` when
  appropriate).
- Keep scope specific (`check`, `localnet`, `stdlib`, `wallet`, `test-runner`,
  `docs`, etc.).
- Use present tense in the subject (for example: `add`, `fix`, `update`, not
  `added`, `fixed`, `updated`).
- Write concise imperative subject in lowercase.
- Keep the subject short (recommended: up to ~72 characters).
- Do not end the subject with a period.
- Optional issue/PR reference at the end is common: `(#123)`.

## Useful environment variables

- `TONCENTER_API_KEY`: API key used by commands that query blockchain data.
- `DISABLE_TMP_DIR_CLEANUP_IN_TESTS=1`: preserve temp test directories.
- `ACTON_LOG_DIR`: custom directory for Acton debug logs.

## AI Policy

Do what you want and how it is convenient for you.
