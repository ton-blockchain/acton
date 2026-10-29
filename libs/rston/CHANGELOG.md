# Changelog

## [Unreleased]

### Fixed

- Cell hashing preserves sparse level masks at each significant level. Nested
  Merkle proofs now produce the same block root hashes as TON nodes.

- LiteClientPool replaces a failed transport on the next query instead of keeping
  a permanently disconnected client. Submitted requests are never replayed, and
  request timeouts still preserve the connection for late responses.

- Single-transaction verification accepts application Merkle proofs and updates
  inside message bodies, as transaction-history pages already did. Pruned data
  outside those cells is still rejected.

### Added

- Global configuration accepts both signed and unsigned 32-bit IPv4 integers
  for liteserver and ADNL endpoints, preserving their signed TL wire encoding.

- TL `BlockIdExt` converts to and from the blockchain `BlockId` model through
  standard conversion traits, checking shard encoding and preserving unsigned
  sequence-number bit patterns. Conversion alone does not authenticate a block.

- `Tokens::from_gram_str` parses decimal GRAM amounts into nanograms exactly,
  rejecting invalid syntax, more than nine fractional digits, and amounts
  outside the 120-bit token range.

## [0.5.0] - 2026-10-14

### Breaking changes

- `BlockSignatures` is now an enum with `Ordinary` and `Simplex` variants.
  Construct ordinary signatures with `OrdinaryBlockSignatures` and access fields
  by matching the variant.

### Added

- LiteClient and LiteClientPool read transaction-history pages from an authenticated
  account cursor or an explicitly trusted cursor. Pages check transaction hashes,
  account identifiers, and logical-time links, return oldest-first records with
  original cells, and expose the authenticated cursor for the next older page.

- LiteClient and LiteClientPool provide block downloads with file and root hash
  checks, authenticated block headers, and block lookup by sequence number,
  logical time, or Unix time. Lookup checks response consistency without
  establishing chain trust. Block-proof options support explicit request-base
  mode (`0x1001`) and weak-target selection.

- `global_config` provides the full TON global configuration model, including
  DHT parameters and signed bootstrap descriptors, liteservers, zero state,
  init block, and hardforks. It requires `models` and `serde` and works without
  networking. `VerifiedBlock::from_trusted_config` selects an explicit trusted
  init block for verified LiteAPI queries.

- High-level LiteClient queries authenticate account state, configuration, and
  transaction inclusion against explicit trusted blocks. Masterchain proof chains
  support Ordinary and Simplex signatures. Get-method results distinguish verified
  source state from server execution, and message submission does not imply
  transaction confirmation.

- `models::vm` provides stack values, cell encoding and decoding,
  continuation data, and snake byte/string helpers for get-method arguments
  and results. Stack types implement `Load` and `Store` for the `VmStack`
  wire format. They work independently of LiteAPI and native TVM execution.

- Optional `tl`, `adnl`, and `liteclient` modules provide LiteAPI wire types,
  ADNL/TCP client and server transport, raw and typed queries, bounded waits,
  multiple liteservers, and global network configuration. Raw and typed wire
  queries return unverified responses.

- Verify Simplex finalization signatures against an expected block and trusted
  validator subset, including candidate binding, signature count and weight,
  validator metadata, and a quorum strictly greater than two thirds.
  `SimplexBlockSignatures::build_data_for_sign()` constructs the signed TL payload.
- Read and write Simplex block signatures (`#12`) in block proofs, retaining the
  original candidate cell chain. Ordinary signatures (`#11`) and the outer
  `BlockProof` format (`#c3`) keep their existing binary representation.

## [0.4.0]

### Breaking changes

- `HashBytes::from_slice()` now returns `Result<HashBytes, TryFromSliceError>`.
  Input lengths other than 32 bytes return an error instead of panicking.

### Added

- TG Wallet rev00 code, deployment data, and transfer bodies, with separate
  address and signing keys for rotation mnemonics.
- Explicit `MnemonicScheme` selection for TON, 12/24-word BIP39, and rotation
  phrases. Rotation phrases derive their anchor and signing keys independently.
- `Boc::decode_any()` decodes a BoC supplied as hex or base64.
- TON wallet code, initial data, address derivation, and signed external messages
  for V2, V3, V4, and V5R1. Each outgoing `WalletMessage` carries its cell and a
  `SendMsgFlags` bitmask. Requests over the wallet's message limit return
  `TooManyMessages` with the actual count and limit. Typed wallet errors identify
  unsupported operations, missing code, and invalid key pairs. `Wallet` provides
  address derivation and signing. Version-specific data and external bodies expose
  `to_cell()` for serialization. Rustdoc describes supported versions, deployment
  defaults, signing requirements, parser behavior, and errors, with protocol sources.
  Signed V5R1 requests support empty transfer lists. Crate and wallet documentation
  include examples for address derivation, transfer signing, and storage decoding.
  `Wallet::state_init()` exposes deployment state for account initialization.
  The wallet type is also available as `rston::Wallet`.
  `wallet::build_ext_in_body()` builds unsigned requests without a private key.
  V5 storage exposes signature authorization as `is_signature_allowed`.
- TON mnemonic validation, English word list, and Ed25519 key derivation, with
  typed `MnemonicError` results and automatic clearing of owned secret data on drop.
  Explicit BIP39 import accepts 12 or 24 words, using SLIP-0010 Ed25519 derivation
  at `m/44'/607'/0'`.

### Changed

- Reject `expect()` calls in the library's Clippy checks.
- Store all four TON hash levels in `AbsentCell` so hash and depth lookup cannot
  index beyond the stored values.
- Return `CellUnderflow` when an included subtree reference is unavailable during
  sequential or parallel Merkle proof construction.
- Require a `0x` or `0X` prefix for 66-character hex hashes.
- Include all benchmark targets in Clippy checks.
- Match TON capability names and masks.
- Accept only TON block and validator constructors. Remove `Block::out_msg_queue_updates` and `ValidatorDescription::mc_seqno_since`.
- Rename `tycho-types` to `rston` and `tycho-types-proc` to `rston-proc`.
- Use stable rustfmt with the shared Acton formatting configuration.
- Disable empty unit-test and documentation-test targets in `rston-proc`.

### Removed

- Unused `typeid` dependency.
- Legacy ABI support: the `abi` feature, `tycho_types::abi` module, and `tycho-types-abi-proc` derive macros.
- Signature-domain models (`SignatureContext` and `SignatureDomain`).
- Tycho-specific block, shard-state, message-queue, and consensus formats, including the `tycho` feature and its consensus/genesis models.
- Optional BLAKE3 helpers and the unused `stats` feature.
