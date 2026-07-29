NEXTEST_PROFILE_ARGS := if env_var_or_default("CI", "") != "" { "-P ci" } else { "" }
TEST_FEATURE_ARGS := if env_var_or_default("CI", "") != "" { "--features only_ci" } else { "" }
SOURCE_TRACE_WASM_OUT := env_var_or_default("ACTON_SOURCE_TRACE_WASM_OUT", "/tmp/acton-source-trace-wasm")
FAUCET_POW_WASM_OUT := env_var_or_default("ACTON_FAUCET_POW_WASM_OUT", justfile_directory() + "/packages/explorer-ui/src/faucet/wasm")

all: precommit

build:
    cargo build --release

build-dev:
    cargo build

build-source-trace-wasm:
    wasm-pack build crates/acton-source-trace-wasm --target web --out-dir "{{ SOURCE_TRACE_WASM_OUT }}" --out-name acton_source_trace_wasm

build-faucet-pow-wasm:
    wasm-pack build crates/acton-faucet-pow-wasm --release --target web --out-dir "{{ FAUCET_POW_WASM_OUT }}" --out-name acton_faucet_pow_wasm
    rm -f "{{ FAUCET_POW_WASM_OUT }}/.gitignore"

sync-artifacts:
    cargo xtask sync-artifacts

install-tools:
    cargo install cargo-shear --version 1.13.1 --locked
    cargo install cargo-deny --version 0.19.8 --locked
    cargo install cargo-audit --version 0.22.1 --locked
    cargo install typos-cli --version 1.47.2 --locked
    cargo install cargo-llvm-cov --locked
    rustup component add llvm-tools-preview
    cargo install wasm-pack --version 0.15.0 --locked

test-unit:
    cargo nextest run --workspace --lib --bins {{ NEXTEST_PROFILE_ARGS }} {{ TEST_FEATURE_ARGS }}
    cargo test --workspace --doc

test-integration:
    cargo nextest run --test integration_test {{ NEXTEST_PROFILE_ARGS }} {{ TEST_FEATURE_ARGS }}

test-workspace:
    cargo nextest run --workspace {{ NEXTEST_PROFILE_ARGS }} {{ TEST_FEATURE_ARGS }}
    cargo test --workspace --doc

install-test-ui-e2e-browsers:
    bun run playwright install chromium

test-ui-e2e-run: install-test-ui-e2e-browsers
    bunx tsc -p packages/test-ui/tsconfig.e2e.json --noEmit
    bun run test:e2e:test-ui

test-ui-e2e: build-ui build-dev test-ui-e2e-run

test-ui-e2e-update: build-ui build-dev install-test-ui-e2e-browsers
    bunx tsc -p packages/test-ui/tsconfig.e2e.json --noEmit
    CHECK_UI_SNAPSHOTS=1 bun run test:e2e:test-ui -- --update-snapshots

_tree-sitter-test grammar:
    cd crates/tree-sitter-{{ grammar }} && yarn install --immutable && yarn tree-sitter generate && yarn tree-sitter test

test-tree-sitter-tolk:
    just _tree-sitter-test tolk

test-tree-sitter-fift:
    just _tree-sitter-test fift

test-tree-sitter-tasm:
    just _tree-sitter-test tasm

test-tree-sitter-tlb:
    just _tree-sitter-test tlb

test-tree-sitter-all: test-tree-sitter-fift test-tree-sitter-tasm test-tree-sitter-tlb test-tree-sitter-tolk

update-test-tree-sitter-tolk:
    cd crates/tree-sitter-tolk && yarn install --immutable && yarn tree-sitter generate && yarn tree-sitter test -u

test: test-workspace

test-update:
    SNAPSHOTS=overwrite just test

docgen:
    cargo run -- docgen

fmt:
    cargo fmt --all

fmt-check:
    cargo fmt --all --check

clippy:
    cargo clippy --workspace --all-targets --all-features --locked -- -D warnings

check-deps:
    cargo shear --deny-warnings

typos:
    typos .

check-docgen:
    cargo run -- docgen --check # always use latest acton

check-docs:
    cd docs && bun ci && bun run generated-source && bun run fmt:check && bun run lint:links:internal && bun run lint:navigation && bun run build

check-schema:
    cargo run -p xtask -- schema --schema acton-toml --check
    cargo run -p xtask -- schema --schema lint-report --check
    cargo run -p xtask -- schema --schema mutation-rules --check

check-deny:
    cargo deny check

check-audit:
    cargo audit

check-templates-security:
  cd src/commands/new/templates/counter-app && npm audit --audit-level=moderate
  cd src/commands/new/templates/empty-app && npm audit --audit-level=moderate
  cd src/commands/new/templates/jetton-app && npm audit --audit-level=moderate
  cd src/commands/new/templates/nft-app && npm audit --audit-level=moderate
  cd src/commands/new/templates/w5-extension-app && npm audit --audit-level=moderate

check-grammar-security:
    cd crates/tree-sitter-fift && yarn npm audit --all --recursive --severity=moderate
    cd crates/tree-sitter-tasm && yarn npm audit --all --recursive --severity=moderate
    cd crates/tree-sitter-tlb && yarn npm audit --all --recursive --severity=moderate
    cd crates/tree-sitter-tolk && yarn npm audit --all --recursive --severity=moderate

check-ui-security:
  bun audit --audit-level=moderate

check-security: check-deny check-audit check-templates-security check-grammar-security check-ui-security
    cd crates/ton-ls/editors/code && yarn npm audit --all --recursive --severity=moderate

check-tolk:
    cargo run -- test
    cargo run -- fmt --check
    cargo run -- check

check-ci: fmt-check check-docgen check-deps clippy typos check-schema check-tolk

check: check-ci check-security test

coverage-setup:
    cargo install cargo-llvm-cov --locked
    rustup component add llvm-tools-preview

coverage:
    cargo llvm-cov --workspace --all-features --all-targets --lcov --output-path lcov.info -- --test-threads 1

coverage-html:
    cargo llvm-cov --workspace --all-features --all-targets --html -- --test-threads 1

coverage-fmt-html:
    cargo llvm-cov -p tolk-fmt --all-features --all-targets --html --open

coverage-clean:
    cargo llvm-cov clean

build-ui:
    bun ci
    cd packages/test-ui && bun ci && bun run build
    cd packages/explorer-core && bun ci && bun run build
    cd packages/studio-ui && bun ci && bun run build

check-ui-ci:
    bun run lint
    bun run fmt:check
    bun run doctor

check-ui: fmt-ui
    bun run doctor
    bun run lint:fix

fmt-ui:
    bun run fmt

play-tree-sitter:
    cd crates/tree-sitter-tolk && yarn install --immutable && yarn tree-sitter generate && yarn tree-sitter build --wasm && yarn tree-sitter playground

update-template-wrappers:
    cargo xtask update-template-wrappers

precommit: fmt fmt-ui build build-ui check check-ui

clean:
    cargo clean
    rm -rf packages/test-ui/dist
    rm -rf packages/studio-ui/dist

generate-schema:
    cargo run -p xtask -- schema --schema acton-toml
    cargo run -p xtask -- schema --schema lint-report
    cargo run -p xtask -- schema --schema mutation-rules
