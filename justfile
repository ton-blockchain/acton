NEXTEST_PROFILE_ARGS := if env_var_or_default("CI", "") != "" { "-P ci" } else { "" }
TEST_FEATURE_ARGS := if env_var_or_default("CI", "") != "" { "--features only_ci" } else { "" }

all: precommit

build:
    cargo build --release

build-dev:
    cargo build

sync-artifacts:
    cargo xtask sync-artifacts

test-unit:
    cargo nextest run --workspace --lib --bins {{ NEXTEST_PROFILE_ARGS }} {{ TEST_FEATURE_ARGS }}
    cargo test --workspace --doc

test-integration:
    cargo nextest run --test integration_test {{ NEXTEST_PROFILE_ARGS }} {{ TEST_FEATURE_ARGS }}

test-workspace:
    cargo nextest run --workspace {{ NEXTEST_PROFILE_ARGS }} {{ TEST_FEATURE_ARGS }}
    cargo test --workspace --doc
    cargo run -- test

test-tree-sitter:
    cd crates/tree-sitter-tolk && yarn install --immutable && yarn tree-sitter generate && yarn tree-sitter test

test-tree-sitter-fift:
    cd crates/tree-sitter-fift && yarn install --immutable && yarn tree-sitter generate && yarn tree-sitter test

test-tree-sitter-tasm:
    cd crates/tree-sitter-tasm && yarn install --immutable && yarn tree-sitter generate && yarn tree-sitter test

test-tree-sitter-tlb:
    cd crates/tree-sitter-tlb && yarn install --immutable && yarn tree-sitter generate && yarn tree-sitter test

test-tree-sitter-all: test-tree-sitter-fift test-tree-sitter-tasm test-tree-sitter-tlb test-tree-sitter

update-test-tree-sitter:
    cd crates/tree-sitter-tolk && yarn install --immutable && yarn tree-sitter generate && yarn tree-sitter test -u

test: test-workspace

test-update:
    SNAPSHOTS=overwrite just test

fmt:
    cargo fmt --all

fmt-check:
    cargo fmt --all --check

clippy:
    cargo clippy --workspace --all-targets --all-features --locked -- -D warnings

check-deps:
    cargo shear

typos:
    typos .

check-docgen:
    cargo run -- docgen --check # always use latest acton

check-schema:
    cargo run -p xtask -- schema --check

check-deny:
    cargo deny check

check-ci: fmt-check check-docgen check-deps clippy typos check-schema

check: check-ci check-deny test

coverage-setup:
    cargo install cargo-llvm-cov --locked
    rustup component add llvm-tools-preview

coverage:
    cargo llvm-cov --workspace --all-features --all-targets --lcov --output-path lcov.info -- --test-threads 1

coverage-html:
    cargo llvm-cov --workspace --all-features --all-targets --html -- --test-threads 1

coverage-fmt-html:
    cargo llvm-cov -p tolkfmt --all-features --all-targets --html --open

coverage-clean:
    cargo llvm-cov clean

build-ui:
    bun ci
    cd crates/acton-test-ui && bun ci && bun run build
    cd crates/acton-litenode-ui && bun ci && bun run build

check-ui-ci:
    bun run lint

check-ui:
    bun run lint:fix

fmt-ui:
    bun run fmt

play-tree-sitter:
    cd crates/tree-sitter-tolk && yarn install --immutable && yarn tree-sitter generate && yarn tree-sitter build --wasm && yarn tree-sitter playground

precommit: fmt fmt-ui build build-ui check check-ui

clean:
    cargo clean
    rm -rf crates/acton-test-ui/dist
