# rston

`rston` is a Rust library for TON cells, BoC serialization, dictionaries, Merkle proofs, and blockchain models.
Use it to read blockchain data, construct messages, and serialize your own types into cells.

It is a TON-focused fork of [tycho-types](https://github.com/broxus/tycho-types), developed in the [Acton repository](https://github.com/ton-blockchain/acton).

The wallet and mnemonic implementations, contract code, and English word list come from [ton-rs](https://github.com/ston-fi/ton-rs/tree/8ee38e8a3c1c034dc78e3d2a6e71aae3d3848790), licensed under MIT or Apache-2.0.

## Installation

`rston` requires Rust 1.88 or later.

Add `rston` to your `Cargo.toml`:

```toml
[dependencies]
rston = "0.3.5"
```

## Basic usage

This example serializes a counter into a cell, encodes it as BoC bytes, and decodes those bytes back into the counter:

```rust
use rston::prelude::*;

#[derive(Load, Store)]
struct Counter {
    value: u32,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cell = CellBuilder::build_from(Counter { value: 42 })?;
    let bytes = Boc::encode(&cell);
    let decoded = Boc::decode(bytes)?;
    let counter: Counter = decoded.parse()?;

    println!("{}", counter.value);
    Ok(())
}
```

Output:

```text
42
```

`Load` and `Store` derive macros implement conversion between Rust types and cells.
Both macros are available through `rston::prelude` without a separate dependency.

## Core types

| Type | Purpose |
| --- | --- |
| `Cell` | An immutable cell with up to 1023 bits and four references |
| `CellBuilder` | Constructs a cell from bits, values, and references |
| `CellSlice` | Reads values and references from an existing cell |
| `Boc` | Encodes and decodes cell trees as BoC bytes |
| `BocRepr` | Encodes and decodes Rust types that implement `Load` and `Store` |
| `Dict`, `AugDict`, `RawDict` | Store values in TON dictionaries |
| `MerkleProof`, `MerkleUpdate` | Represent proofs and updates for cell trees |

The `models` module includes addresses, messages, accounts, transactions, blocks, shard states, and blockchain configuration.
For custom cell formats, `CellBuilder` and `CellSlice` provide operations for individual fields.

The `mnemonic` module validates 24-word TON mnemonics and derives Ed25519 key pairs.
It exposes `Mnemonic`, `KeyPair`, and the English `WORDLIST_EN_SET`.

The `wallet` module derives wallet addresses and prepares signed external messages
using `rston` cells. It exposes `Wallet`, `WalletVersion`, wallet data types,
and contract code. Message signing supports V2, V3, V4, and V5R1.
Pass outgoing messages as `Vec<WalletMessage>`. Each entry contains the complete
message cell in `msg` and a `SendMsgFlags` bitmask in `mode`.
Combine flags with `|`, such as `SendMsgFlags::PAY_FEE_SEPARATELY | SendMsgFlags::IGNORE_ERROR`.

## Features

The default features are `base64`, `serde`, `models`, `sync`, and `wallet`.

| Feature | Adds |
| --- | --- |
| `base64` | Base64 helpers for BoC and byte representations |
| `serde` | Serde serialization for supported types. Enables `base64` |
| `models` | TON blockchain models |
| `wallet` | Wallet contracts and signed messages. Enables `models`, `base64`, and `sync` |
| `sync` | Cells that can be shared across threads |
| `bigint` | Conversion helpers for `num-bigint` integers |
| `rayon` | Parallel BoC encoding and Merkle operations. Enables `sync` |
| `rand8`, `rand9` | Random value generation through the corresponding `rand` version |
| `arbitrary` | Value generation for fuzz tests |

With `default-features = false`, cells use the single-threaded implementation.
Cell builders, slices, BoC bytes, and dictionaries remain available.

## API documentation

See the [API reference on docs.rs](https://docs.rs/rston) for modules, type definitions, and more examples.

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for tests, benchmarks, formatting, Miri, and fuzzing commands.

## License

Licensed under either of

* Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or <https://www.apache.org/licenses/LICENSE-2.0>)
* MIT license ([LICENSE-MIT](LICENSE-MIT) or <https://opensource.org/licenses/MIT>)

at your option.
