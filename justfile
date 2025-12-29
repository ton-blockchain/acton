all: precommit

build:
    cargo build

build-release:
    cargo build --release

i-test:
    cargo test --test integration_test

d-test:
    cargo test --test debug_test -- --test-threads 1

test:
    cargo test -p abi -p dap-client -p emulator -p tolk_parser -p ton-api -p tvmffi -p vmlogs \
    && cargo test -p retrace -- --test-threads 1 \
    && cargo test --lib commands::up::tests \
    && cargo test --lib config::tests \
    && cargo test --lib file_build_cache::tests \
    && just i-test \
    && just d-test

test-update:
    SNAPSHOTS=overwrite just test

fmt:
    cargo fmt --all

clippy:
    cargo clippy --workspace --all-features --all-targets -- -D warnings

check: fmt clippy test

clean:
    cargo clean

precommit:
    just build && just check
