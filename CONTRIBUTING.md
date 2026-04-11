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

## Command conventions

- Unless stated otherwise, run commands from the repository root.
- Prefer `just` targets over ad-hoc commands when both exist.
- Some targets intentionally modify files (`just check-ui`, `just fmt-ui`,
  `just test-update`).

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
- `cargo-deny` (dependency policy checks for `just check`)
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
   from the `release-objs` release and, on a fresh checkout, downloads the
   matching prebuilt `objs/` archive for your current platform.
2. `just build-ui` installs UI dependencies and builds the bundled UI assets.
3. `just build-dev` builds the debug CLI against the synced TON archives.
4. `./target/debug/acton --help` confirms that the binary starts.

If `objs/` already exists and the tracked manifest changed, `just
sync-artifacts` may ask whether local `objs/` should be refreshed. Use
`just sync-artifacts --force` to refresh without a prompt.

## Building from source

Acton links static TON artifacts (`libemulator.a`, `libtolk.a`) from the
`i582/ton` fork branch `pmakhnev/acton`.

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
- updates `crates/ton-objs/artifacts_manifest.toml` when needed;
- downloads and unpacks the matching `ton-objs-<target>.tar.gz` archive into
  `objs/` on a fresh checkout;
- downloads a temporary `ton-stdlib.tar.gz`, replaces
  `crates/tolkc/assets/tolk-stdlib/` and `crates/tolkc/assets/fift-stdlib/` from its
  `tolk-stdlib/` directory and `fift-stdlib/Asm.fif` plus
  `fift-stdlib/Fift.fif` whenever local `objs/` are refreshed, then removes
  the temporary archive;
- can also refresh local `objs/` later when the tracked manifest changes.

Use `just sync-artifacts --force` if you want to overwrite the local
manifest and refresh `objs/` without confirmation.

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
# edit crates/ton-objs/artifacts_manifest.toml:
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
- `retrace` tests stay inside the shared workspace run, but are serialized via
  a nextest test group because they hit rate-limited remote APIs.
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

Documentation site (Next.js in `docs/`, package manager: Yarn via Corepack):

```bash
corepack enable
cd docs
yarn install --immutable --check-cache --check-resolutions
yarn dev
```

Build docs:

```bash
corepack enable
cd docs
yarn build
```

Regenerate auto-generated MDX documentation from Acton sources:

```bash
./target/debug/acton docgen
# or
cargo run --bin acton -- docgen
# check mode (fails with exit code 1 and prints diff if docs are stale)
cargo run --bin acton -- docgen --check
```

This updates generated docs under:

- `docs/content/docs/standard_library`
- `docs/content/docs/tolk_standard_library`
- `docs/content/docs/linting/rules`

If your PR changes any docgen inputs, running `acton docgen` and committing
generated documentation changes is required. This includes:

- `lib/`
- `crates/tolkc/assets/tolk-stdlib/`
- linter rule metadata and mappings (for example `crates/tolk-linter/`,
  `crates/tolk-macros/`)

After doc updates (manual or generated), validate docs build:

```bash
corepack enable
cd docs
yarn install --immutable --check-cache --check-resolutions
yarn build
```

## Tree-sitter workflows

If your PR changes any `crates/tree-sitter-*` grammar/parser artifacts:

```bash
just test-tree-sitter-all
```

For quick Tolk-only iteration:

```bash
just test-tree-sitter
```

When grammar snapshots need refresh:

```bash
just update-test-tree-sitter
```

## Change-based checklist

Use this as a quick local matrix before pushing:

| Change type                                                                                        | Required local checks                                                                                     |
|----------------------------------------------------------------------------------------------------|-----------------------------------------------------------------------------------------------------------|
| Rust-only code                                                                                     | `just check`                                                                                              |
| UI code (`crates/acton-*-ui`, root `package.json`)                                                 | `just check` + `just build-ui` + `just check-ui`                                                          |
| Standard library / docgen inputs (`lib/`, `crates/tolkc/assets/tolk-stdlib`, linter rule metadata) | `just check` + `acton docgen` and  commit generated docs                                                  |
| Docs site content/config (`docs/`)                                                                 | `corepack enable && cd docs && yarn install --immutable --check-cache --check-resolutions && yarn build`  |
| Tree-sitter grammar (`crates/tree-sitter-*`)                                                       | `just check` +`just test-tree-sitter-all` (and `just update-test-tree-sitter` when Tolk snapshots change) |
| Release preparation (maintainers)                                                                  | Follow [RELEASING.md](RELEASING.md)                                                                       |

## PR requirements

Every pull request must pass all checks from:

```bash
just check
```

This command runs Rust formatting, docgen, dependency, dependency-policy, lint, schema, and test checks.
Install `cargo-shear`, `cargo-deny`, and `typos-cli` if you want to run it locally.
`typos` uses `_typos.toml` excludes for `docs/` and selected generated or imported trees.

If your PR touches UI code (`crates/acton-test-ui`, `crates/acton-litenode-ui`,
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
- Keep scope specific (`check`, `litenode`, `stdlib`, `wallet`, `test-runner`,
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
