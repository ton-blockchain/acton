# Acton Rust prototype

This directory contains the first vertical slice of a Rust DSL for TON contracts.

The source is valid Rust and can be checked with Cargo. `acton-rustc` reads the same source and lowers the supported DSL subset to ordinary Tolk. The generated Tolk remains compatible with the existing Acton compiler and tooling.

## Try the counter

Check the Rust source:

```shell
cargo check -p counter-contract
```

Generate Tolk:

```shell
cargo run -p acton-rust-compiler --bin acton-rustc -- \
  examples/counter/src/lib.rs \
  --output /tmp/Counter.tolk
```

Compile it with Acton:

```shell
acton compile /tmp/Counter.tolk --boc /tmp/Counter.boc
```

Run the snapshot test:

```shell
cargo test -p acton-rust-compiler
```

## Supported subset

- One inline module annotated with `#[contract(...)]`
- One `#[storage]` struct
- Struct messages annotated with `#[message(op = ...)]`
- `#[receive]` handlers with `Context<State>` and one message argument
- Direct storage assignments and assignment operators
- Parameterless `#[get]` methods with `ViewContext<State>`
- Primitive `bool`, signed integer, and unsigned integer fields
- Deterministic snake_case to lowerCamelCase lowering

Unsupported contract expressions and types fail with an explicit compiler error. The next useful milestone is the Jetton wallet because it adds typed cells, addresses, coins, sends, bounce handling, and StateInit without requiring the low-level wallet and system-contract escape hatches yet.
