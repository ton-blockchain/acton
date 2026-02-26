CARGO_TEST := `if cargo nextest --version >/dev/null 2>&1; then echo "cargo nextest run"; else echo "cargo test"; fi`
TEST_SERIAL_ARGS := `if cargo nextest --version >/dev/null 2>&1; then echo "--test-threads 1"; else echo "-- --test-threads 1"; fi`

all: precommit

build:
    cargo build --release

test-unit:
    {{ CARGO_TEST }} --workspace --lib --bins \
        --exclude retrace
    cargo test --workspace --doc

test-serial:
    # we need test by test execution due to Toncenter rate limit
    {{ CARGO_TEST }} -p retrace {{ TEST_SERIAL_ARGS }}

test-integration:
    {{ CARGO_TEST }} --test integration_test
    # we need test by test execution due to single debug port
    # {{ CARGO_TEST }} --test debug_test {{ TEST_SERIAL_ARGS }}

test-tree-sitter:
    cd crates/tree-sitter-tolk && yarn && tree-sitter generate && tree-sitter test

update-test-tree-sitter:
    cd crates/tree-sitter-tolk && yarn && tree-sitter generate && tree-sitter test -u

test: test-unit test-serial test-integration test-tree-sitter

test-update:
    SNAPSHOTS=overwrite just test

fmt:
    cargo fmt --all

fmt-check:
    cargo fmt --all --check

clippy:
    cargo clippy --workspace --all-targets --all-features --locked -- -D warnings

check-udeps:
    cargo +nightly udeps --workspace

check: fmt-check clippy test

coverage-setup:
    cargo install cargo-llvm-cov
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
    bun install
    cd crates/acton-test-ui && bun i && bun run build
    cd crates/acton-litenode-ui && bun i && bun run build

check-ui:
    bun run lint:fix

fmt-ui:
    bun run fmt

play-tree-sitter:
    cd crates/tree-sitter-tolk && yarn && npx tree-sitter generate && npx tree-sitter build --wasm && npx tree-sitter playground

precommit: fmt fmt-ui build build-ui check check-ui

clean:
    cargo clean
    rm -rf crates/acton-test-ui/dist
