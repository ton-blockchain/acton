all: precommit

build:
    cargo build

build-release:
    cargo build --release

i-test:
    # run integration tests and auto update snapshots
    cargo test --test integration_test

d-test:
    # run debugger tests and auto update snapshots
    cargo test --test debug_test -- --test-threads 1

test:
    cargo test -p abi -p dap-client -p emulator -p tolk_parser -p ton-api -p tvmffi -p vmlogs\
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
