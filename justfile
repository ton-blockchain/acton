all: precommit

build:
    cargo build

build-release:
    cargo build --release

i-test:
    # run integration tests and auto update snapshots
    SNAPSHOTS=overwrite cargo test --test integration_test

d-test:
    # run debugger tests and auto update snapshots
    RUSTFLAGS="-Awarnings" SNAPSHOTS=overwrite cargo test --test debug_test -- --test-threads 1

test:
    cargo test -p abi -p dap-client -p emulator -p tolk_parser -p ton-api -p tvmffi -p vmlogs\
    && just i-test \
    && just d-test

fmt:
    cargo fmt --all

clippy:
    cargo clippy --workspace --all-features --all-targets -- -D warnings

check: fmt clippy test

clean:
    cargo clean

precommit:
    just build && just check
