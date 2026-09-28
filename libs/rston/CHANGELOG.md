# Changelog

## [Unreleased]

### Added

- TON wallet code, initial data, address derivation, and signed external messages
  for V2, V3, V4, and V5R1.
- TON mnemonic validation, English word list, and Ed25519 key derivation, with
  typed `MnemonicError` results and automatic clearing of owned secret data on drop.

### Changed

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
