# TonCenter API Compatibility Testing Strategy

This document defines a testing strategy for increasing confidence that Acton
localnet behaves like the TON services and clients it replaces during local
development. It complements the current
[API compatibility matrix](content/docs/localnet/api-compatibility.mdx) and
[LiteAPI support matrix](content/docs/localnet/liteapi-support.mdx).

The strategy is intentionally layered. A response can match the TonCenter JSON
schema while containing incorrect blockchain data, and correct blockchain data
can still be encoded in a way that breaks an existing client.

## Compatibility Dimensions

Compatibility should be measured independently in four dimensions:

| Dimension              | What must match                                                                                                  |
| ---------------------- | ---------------------------------------------------------------------------------------------------------------- |
| Wire compatibility     | HTTP methods and statuses, headers, JSON envelopes, field presence, scalar types, encodings, and array ordering. |
| TON semantics          | Account state, balances, hashes, logical times, cells, transaction phases, TVM results, and block relationships. |
| Temporal semantics     | Visibility after submission, pending state, ordering, pagination, finality, and streaming delivery.              |
| Consumer compatibility | Observable behavior through supported SDKs and command-line clients.                                             |

Passing one dimension must not be used as evidence that another dimension is
correct.

## Pin the Compatibility Target

"TonCenter compatible" is not a stable target unless the reference versions are
recorded. Compatibility tests should pin:

- An exact tag or commit of the official
  [`ton-http-api-cpp`](https://github.com/toncenter/ton-http-api-cpp) reference for
  API v2.
- A saved copy of the official
  [TonCenter v2 OpenAPI schema](https://toncenter.com/api/v2/openapi.json).
- An exact tag or commit of the official
  [`ton-indexer`](https://github.com/toncenter/ton-indexer) reference for API v3.
- The supported Streaming API behavior from the
  [official streaming documentation](https://docs.ton.org/api/streaming/overview).

The target and supported subset should live in a machine-readable compatibility
manifest. Each operation should identify its transports, oracle, tests, and
known deviations.

```yaml
targets:
  toncenter_v2:
    implementation: toncenter/ton-http-api-cpp
    revision: "<tag-or-commit>"
    openapi: "<saved-schema-path>"
  toncenter_v3:
    implementation: toncenter/ton-indexer
    revision: "<tag-or-commit>"

operations:
  - name: getAddressInformation
    transports:
      - rest_get
      - rest_post
      - json_rpc
    oracle: same_state_differential
    known_deviations: []
```

Unsupported operations and unsupported parameters must be explicit. They should
not silently return plausible but incomplete data.

## Test Layers

No single oracle is sufficient. The recommended suite combines the following
layers.

| Layer                          | Primary oracle                                | Recommended schedule |
| ------------------------------ | --------------------------------------------- | -------------------- |
| OpenAPI conformance            | Pinned upstream schemas                       | Every pull request   |
| Same-state API v2 differential | Official HTTP API over Acton LiteAPI          | Every pull request   |
| Immutable real-chain replay    | Raw BoCs, proofs, and pinned hosted responses | Nightly              |
| Cross-endpoint invariants      | Independently decoded TON data                | Every pull request   |
| SDK compatibility              | Supported production clients                  | Every pull request   |
| Differential fuzzing           | Official implementation and local invariants  | Nightly              |
| Hosted API canaries            | Mainnet and testnet TonCenter                 | Weekly, non-blocking |

### Optional Live Contract Tests

The `ton-api` crate contains an ignored live suite in
`crates/ton-api/tests/toncenter_live_contract.rs`. It sends the typed request
DTOs to the public TonCenter v2, v3, emulation, SSE, and WebSocket endpoints and
deserializes responses directly into the corresponding typed response DTOs.
Repeated query parameters, aliases, cursors, ranges, sorting, optional flags,
success envelopes, and error envelopes are covered.

The suite is gated twice so that a normal `cargo test` never depends on the
network:

```shell
ACTON_TONCENTER_LIVE=1 \
ACTON_TONCENTER_LIVE_API_KEY="<key>" \
cargo test -p ton-api --test toncenter_live_contract -- \
  --ignored --test-threads=1
```

`TONCENTER_API_KEY` is accepted as a fallback. The key is required for the
streaming tests and recommended for the HTTP tests. Requests are rate-limited
inside the test process and HTTP 429/5xx responses receive a small bounded
retry. Run the test binary with one test thread so the external load and failure
reports remain deterministic.

The following overrides allow the same suite to exercise testnet, a proxy, or a
staging deployment:

- `ACTON_TONCENTER_LIVE_V2_URL`
- `ACTON_TONCENTER_LIVE_V3_URL`
- `ACTON_TONCENTER_LIVE_EMULATE_URL`
- `ACTON_TONCENTER_LIVE_SSE_URL`
- `ACTON_TONCENTER_LIVE_WEBSOCKET_URL`

Successful emulation tests additionally accept
`ACTON_TONCENTER_LIVE_EMULATE_BOC` and
`ACTON_TONCENTER_LIVE_TONCONNECT_JSON`. Without these fixtures, the suite still
checks the real validation-error contracts. Message submission tests always use
an invalid BOC and never broadcast a valid message to mainnet.

Live responses are intentionally not snapshots: block heights, hashes, and
pending state change continuously. A test passes only when the actual response
matches a typed success or documented typed error contract. Schema mismatches
include the target Rust type, HTTP status, and an abbreviated response body in
the failure message.

### OpenAPI Conformance

The conformance suite should validate the advertised subset of every API family:

- REST `GET`, REST `POST`, and JSON-RPC forms where the reference exposes them.
- Required parameters, defaults, accepted aliases, and upper and lower bounds.
- Success and error envelopes, HTTP statuses, and content types.
- String representation of 64-bit integers and base64, base64url, and hex fields.
- Missing fields versus explicit `null` values.
- Invalid addresses, hashes, BoCs, method names, stacks, block identifiers, and
  conflicting filters.

A separate upstream-drift job should download the latest schemas and report a
diff. It must not update pinned schemas or snapshots automatically.

### Same-State Differential Testing for API v2

This is the highest-value compatibility test for API v2:

1. Start a deterministic Acton localnet with a known state and virtual time.
2. Point the pinned official `ton-http-api-cpp` service at the Acton LiteAPI
   endpoint.
3. Send the same request corpus to the Acton HTTP API and the official HTTP API.
4. Compare the responses using both strict-wire and semantic comparators.

Both HTTP implementations observe the same accounts, blocks, and transactions,
so a difference is not caused by querying two networks at different heights.
This makes the setup a strong HTTP and LiteAPI compatibility oracle, but not an
independent oracle for the correctness of the underlying localnet state. The
real-chain fixtures and cross-endpoint invariants cover that separate question.

The official implementation may expose a LiteAPI limitation by requesting a
method or proof that localnet does not yet provide. That result should be
recorded as a LiteAPI compatibility gap, not hidden by weakening the HTTP
comparison.

Failures can be localized by retaining artifacts from four boundaries:

1. The localnet internal state or generated block cells.
2. Raw Acton LiteAPI requests and responses.
3. Official TonCenter HTTP responses built from those LiteAPI responses.
4. Acton TonCenter-compatible HTTP responses.

### Immutable Real-Chain Replay

Real-chain fixtures should be addressed by immutable identifiers rather than by
"latest" state. A fixture should include:

- Full `BlockIdExt`: workchain, shard, seqno, root hash, and file hash.
- Raw block, account, transaction, and message BoCs required by the scenario.
- LiteAPI proofs when the queried operation has a proof-bearing source.
- Hosted TonCenter v2 and v3 responses captured for the same immutable data.
- The upstream version and capture date.

API v2 responses can be checked against raw LiteAPI data and independently
decoded cells. API v3 requires a separate indexer oracle because it provides
derived queries, traces, assets, and historical filtering. Pull requests can use
recorded v3 fixtures.

The official v3 indexer reads full-node data into PostgreSQL and cannot be
pointed directly at Acton LiteAPI. A stronger nightly job therefore needs an
upstream-supported fixture or import path that populates the reference indexer
database, followed by the same API queries against that database and Acton's
recorded fixture state.

Fixtures should cover old enough blocks that normal chain progress cannot change
the result. Off-chain metadata should use controlled fixtures or be tested
separately from canonical on-chain values.

### Cross-Endpoint Invariants

Invariants detect internally inconsistent behavior and reduce dependence on a
single reference implementation. At minimum, verify that:

- `getAddressBalance` equals the balance in `getAddressInformation`.
- v2 and v3 `runGetMethod` agree on exit code, gas, and the decoded TVM stack.
- v2 transaction history, standard history, and v3 transactions agree on
  account, hash, logical time, and message relationships.
- Every transaction returned by a block listing resolves through account
  history and points back to the same block.
- A parent out-message hash equals the corresponding child in-message hash.
- `sendBocReturnHash` agrees with a locally computed message hash.
- Account code and data BoCs decode and match their advertised representation
  hashes.
- Pagination over a frozen state has no gaps or duplicates, and ascending and
  descending views are mutually consistent.
- Trace parent and child edges agree with transaction message links.

These checks should decode TON structures through the normal cell and TL-B
libraries rather than through a second copy of the response-mapping code.

### Consumer Compatibility

Run end-to-end smoke scenarios through the same clients used by real projects.
Useful clients include `@ton/ton` `TonClient`, TonWeb, and a raw LiteAPI client
such as `tonutils-go`.

A client scenario should perform a complete workflow where possible:

1. Read the initial account state.
2. Submit an external message.
3. Wait for visibility or confirmation using the client's normal mechanism.
4. Read the updated balance and account state.
5. Run a get method.
6. Find the resulting transaction and trace.

This layer catches transport and representation details that are technically
valid JSON but incompatible with client expectations.

### Property and Differential Fuzz Testing

Property tests should cover values that are easy to underrepresent in example
fixtures:

- Raw, bounceable, non-bounceable, and test-only address forms.
- Valid and invalid address checksums.
- Hex, base64, and base64url transaction and message hashes.
- Zero, negative, and large TVM integers.
- Cell, slice, tuple, list, and null stack values.
- Empty, malformed, deeply nested, and oversized BoCs.
- Filter combinations, repeated parameters, pagination boundaries, and sort
  directions.

Where possible, send minimized fuzz cases to both Acton and the pinned reference
implementation. Regardless of reference behavior, malformed input must not cause
a panic, unbounded allocation, or an unbounded request.

### Temporal and Streaming Behavior

Temporal tests should exercise the transition from message submission to indexed
and streamed state:

- Query immediately before and after message submission.
- Observe pending and committed transactions where supported.
- Verify transaction and account-state event ordering.
- Reconnect SSE and WebSocket clients and document whether gaps or duplicates are
  possible.
- Reconcile streaming events with a subsequent API v3 query.
- Test multiple transactions in one block and multiple transactions for one
  account.

Use controlled virtual time and deterministic mining where possible. Latency
itself is not a compatibility requirement unless a documented timeout or
delivery guarantee depends on it.

## Comparison Policy

Tests should retain two different comparisons instead of one permissive
normalizer.

### Strict-Wire Comparison

Strict-wire comparison includes:

- HTTP status and content type.
- JSON field presence and scalar type.
- JSON-RPC ID value and type.
- Integer, address, and hash encoding.
- Array order when the API defines ordering.
- Success and error envelope shape.

JSON object key order is irrelevant. Patch-specific error prose may be classified
separately when the upstream contract only guarantees the code and envelope.

### Semantic Comparison

Semantic comparison may decode and canonicalize:

- Friendly addresses into workchain and 256-bit account identifiers.
- Hashes from hex, base64, or base64url into 256-bit values.
- BoCs into cell graphs and representation hashes.
- TVM stack entries into typed values.

Different valid BoC container serializations should be compared through decoded
cells and representation hashes. Exact base64 remains part of strict-wire
comparison only when the pinned API contract requires a canonical encoding.

Fields such as request-processing time, server version, and volatile `@extra`
metadata may be normalized explicitly. The allowlist of normalized fields must
be reviewed and kept small.

Comparing independently executed localnet and testnet scenarios requires a
semantic projection. Block hashes, logical times, timestamps, random seeds, and
absolute fees can differ because network config and execution context differ.
They must not be removed from same-state differential tests.

## Scenario Catalog

The fixture catalog should grow by semantic risk rather than only by endpoint
count.

| Area                 | Required scenarios                                                                                                    |
| -------------------- | --------------------------------------------------------------------------------------------------------------------- |
| Address utilities    | Every supported address form, checksum failures, pack/unpack round trips, and hash encodings.                         |
| Account lifecycle    | Non-existent, uninitialized, active, frozen, and deleted accounts with and without code/data.                         |
| Execution            | Successful, rejected, aborted, bounced, destroyed, deploy-with-state-init, and no-compute transactions.               |
| Get methods          | All supported stack types, non-zero exits, missing methods, historical block execution, and config-dependent getters. |
| History              | Multiple transactions per account, cursor boundaries, block listings, source/result lookup, and pagination.           |
| Messages and traces  | External-in, internal, external-out, empty bodies, opcodes, multi-hop traces, and bounced links.                      |
| Config and libraries | Present and missing config parameters, known and unknown libraries, and historical lookups.                           |
| Assets               | Jetton master/wallet and NFT item/collection data with on-chain, off-chain, missing, and invalid metadata.            |
| Failures             | Malformed input, unknown resources, conflicting filters, authentication, rate limits, and request limits.             |

Localnet does not emulate every validator or shardchain behavior. Validator
signatures, general consensus proof chains, shard split/merge history, and other
unsupported features should remain explicit exclusions until they are modeled.

## CI Schedule

### Pull Requests

Run deterministic checks that do not depend on public services:

- Pinned OpenAPI conformance and compatibility-manifest coverage.
- Same-state API v2 differential tests.
- Normalized response snapshots for deterministic fixtures.
- Cross-endpoint invariants.
- SDK smoke tests.
- A bounded property-test corpus.

### Nightly

Run heavier checks:

- Immutable real-chain fixture replay with raw-cell validation.
- Pinned API v3 indexer and database tests.
- Extended differential fuzzing.
- Long-running temporal, pagination, and streaming scenarios.

### Weekly Canaries

Query hosted mainnet and testnet TonCenter using immutable identifiers and compare
the results with pinned expectations. Also report changes in upstream OpenAPI
schemas and reference releases.

Hosted canaries should notify maintainers rather than block pull requests. Rate
limits, provider outages, and upstream deployments are not local code failures.

## Recommended Implementation Order

1. Add the compatibility manifest and pin the v2 and v3 references.
2. Add OpenAPI validation and an upstream-schema drift report.
3. Build the same-state v2 harness around the existing localnet LiteAPI endpoint.
4. Cover a small high-value corpus: address utilities, account information,
   balance/state, masterchain information, get methods, transaction history,
   block transactions, message submission, and v3 transactions/traces.
5. Add cross-endpoint invariants and SDK workflow tests.
6. Record immutable mainnet and testnet fixtures with raw BoCs and proofs.
7. Add the nightly v3 indexer environment and differential fuzzing.

The acceptance criterion is not zero raw diffs. It is zero unclassified diffs:
every difference must either be fixed, documented as an intentional localnet
deviation, or represented as an unsupported capability in the compatibility
manifest.
