# Wallet parsing fixtures

These fixtures test wallet parsing without network access.
The BoCs contain public contract code, mainnet account data, deployment states, and signed message bodies.

[`manifest.json`](manifest.json) records each source URL, wallet revision, address, and cell hash.
Account records also include the last transaction hash and logical time.
Message records include the transaction hash and full message hash; their BoC files contain only the signed body.
The capture date and ABI catalog commit are recorded at the top of the manifest.
Current-state endpoints can return newer data later. Use the recorded hashes and logical times to identify the captured state.

## Coverage

| Fixtures | Coverage |
| --- | --- |
| 15 contract code cells | Every `WalletVersion`, checked against the pinned TON ABI catalog and embedded code |
| 28 storage and deployment cells | All 11 revisions with initial-data support: V1, V2, V3, V4, V5R1, and Highload V2R2 |
| 19 signed requests | Both V2 revisions, both V3 revisions, both V4 revisions, and V5R1 |

Storage cases include zero and nonzero counters, different public keys, and masterchain and basechain deployment addresses.
V4 cases include empty and populated plugin dictionaries.
V5 cases include zero, one, and five extensions, with signature authentication enabled or disabled.
Highload V2R2 cases include empty and populated query dictionaries and a nonzero cleanup watermark.

Request cases include modes `3`, `128`, and `130`, mixed modes, and batches of 1, 2, 4, and 100 messages.
They cover empty payloads, comments, jetton transfers, contract deployment, and payload references.
The four-message and hundred-message batches are V5 requests; the two-message batch also has a V2R2 case.
Expiration values include ordinary timestamps and `uint32::MAX`.

The tests check full consumption and cell-hash preservation after serialization.
They also verify each request's Ed25519 signature against the captured account's public key.
JSON snapshots preserve storage fields, dictionary entries, request headers, send modes, and outgoing message order.

## Current limits

Highload V1R1, V1R2, V2, and V2R1 have code-identification fixtures only.
No mainnet addresses were found for the two Highload V1 revisions during this collection.
V1 and Highload message bodies have no typed parser in the wallet module.
V4 plugin-management operations and V5 extended actions are also outside the current transfer API.
These fixtures do not claim coverage for those operations or every protocol boundary.

## Run the tests

Run from `libs/rston`:

```sh
cargo test --test wallet_fixtures --all-features
```

To update snapshots after an intentional change:

```sh
UPDATE_EXPECT=1 cargo test --test wallet_fixtures --all-features
```

Review snapshot changes against the source BoCs before accepting them.
