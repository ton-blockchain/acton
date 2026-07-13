# Localnet API risk register

Status: active remediation of the initial static audit, 2026-07-13.

This document ranks the compatibility risk and semantic test coverage of every HTTP route exposed
by `ton-localnet`. It is a bug-finding register, not an API availability matrix. Availability is
documented in `docs/content/docs/localnet/api-compatibility.mdx`; the general differential-testing
strategy is documented in `docs/TONCENTER_API_COMPATIBILITY_TESTING.md`.

## Audit baseline

The comparison used these source revisions:

- localnet: `9efa89d57acf42fd653b717dbc818e7eac6d0076`;
- TonCenter V2 oracle: `ton-http-api-cpp` at
  `651e81dff5b8cff9a884db7ab274326e7ae5d43d`;
- TonCenter V3 oracle: `ton-indexer` at
  `bd15a121b3a450f34546176ada3ca475db045704`;
- request and response schemas: the generated `ton-api` DTOs and the upstream OpenAPI documents.

This pass is a source-level differential audit. A mismatch marked **confirmed** follows directly
from both implementations. A **high-confidence suspicion** still needs a differential fixture
because the upstream implementation, schema, or underlying Lite API leaves room for ambiguity.
A **coverage gap** is not itself evidence of a bug.

Known and accepted product limitations are not counted as new defects:

- V3 action classification is not implemented;
- `include_on_sale` cannot be complete without persistent sale classification;
- fork mode does not populate local V3 indexes from V2 account history;
- streaming actions are empty and `trace_invalidated` is not emitted without a reorg model;
- POST duplicates for read-only V2 GET endpoints are intentionally not exposed;
- endpoints explicitly listed as unsupported in the compatibility matrix remain out of scope.

The invalid V3 value `classification_state="classified"` and protocol validation differences are
still defects because they violate schemas even under those limitations.

## Remediation status

The 65 actionable findings are classified by the highest-risk route or cross-cutting surface they
affect. The two accepted V2 deviations are excluded; unresolved streaming oracle conflicts remain
open until their normative contract is chosen.

| Risk | Fixed | Partial | Open |
|---|---:|---:|---:|
| Critical | 12 | 5 | 7 |
| High | 15 | 4 | 18 |
| Medium | 0 | 1 | 3 |
| **Total** | **27** | **10** | **28** |

| Finding | Status | Evidence |
|---|---|---|
| V2-01 `runGetMethodStd` | Fixed | Shared `tvm-ffi` Tonlib stack DTOs, separate Std request/result types, local REST/JSON-RPC snapshot, and a live non-empty TonCenter stack test. |
| V2-02 JSON-RPC envelope | Fixed | The incoming proxy DTO accepts method-only requests and upstream params normalization; local and live tests cover ignored metadata and every params shape. |
| V2-03 JSON-RPC `getShards` | Fixed | Dispatch now uses the upstream method name; the non-upstream `shards` name is rejected and local/live typed responses are covered. |
| V2-04 account status | Fixed | REST and JSON-RPC use the same V2 status mapper; a no-state account is covered through both transports. |
| V2-06 validation errors | Partial | Shared V2 validation, JSON-RPC field parsing, block request parsers, and transaction lookup validation now return typed envelopes. Axum extractor rejections remain open. |
| V2-07 positive `seqno` | Fixed | All account, shard-account, wallet, token, and config entry points reject explicit zero, negative, and values above signed int32; REST and JSON-RPC are snapshot-covered. |
| V2-08 token history | Fixed | `getTokenData` detects assets from the account state at the requested block. Real jetton supply/balance and NFT collection/owner transitions are snapshot-covered through typed REST and JSON-RPC responses. |
| V2-09 config history | Fixed | Each masterchain block stores its config hash; config reads and rebuilt historical states use that hash. A real config-param mutation across blocks is covered. |
| V2-10/11 transaction history | Fixed | Account history excludes `to_lt`, requires an exact nonzero cursor, and accepts hex, standard base64, padded base64url, and unpadded base64url hashes. Real REST/JSON-RPC history and invalid cursors are snapshot-covered. |
| V2-12 transaction DTO | Partial | Decoded endpoints recognize text comments across snake references and expose legacy decode fields, while Std/block-ext endpoints remain raw. V2 and V3 preserve uint32 currency IDs and full VarUint248 amounts. Encrypted/decrypted message classification remains open. |
| V2-13 extended account state | Partial | Code-less uninit/frozen/active accounts, V3/V4, Highload V1/V2, and Manual DNS states are typed and snapshot-covered, including pinned revisions, unsigned wallet IDs, signed seqnos, raw/friendly address flags, historical `sync_utime`, and full-width extra currencies. Recognized malformed state data is an error instead of a raw success. RWallet and PChan decoders remain open. |
| V2-14 wallet DTO | Fixed | V1-V5 and the upstream V5-beta hash are detected from the decoded code BOC and read with the same storage prefixes as C++, with signed seqno, wallet ID, signature flag, and exact optional-field omission. Real V2-V5 startup wallets plus prefix-only/malformed data, stale hash, highload-negative, and no-state cases are covered; V1 shares the tested V1/V2 parser path. |
| V2-15 token DTOs | Partial | Mintless claim state, the typed DNS content union, and direct NFT collection detection now match the C++ response contract. Parent-contract NFT verification and collection-derived item content remain open. |
| V2-16 block headers | Fixed | The response is populated from the serialized block's `BlockInfo`, including the real global ID, key-block fields, and previous block IDs. Basechain/masterchain headers and single/both hash selectors are snapshot-covered. |
| V2-17 block transactions | Fixed | Zero and unknown cursors start at the first transaction, exact cursors are exclusive, short transaction mode is 135, single block hashes are lookup hints, and validation is typed 422. Generated localnet masterchain blocks have no account blocks, so their empty result is faithful to the source block. |
| V2-18 block lookup | Fixed | REST and JSON-RPC require exactly one signed-range selector; seqno, LT, time, zero values, missing selectors, and combined selectors are snapshot-covered. |
| V2-21 historical account fields | Partial | Account responses use the selected block's generation time, including historical reads. The unsupported `suspended` account flag remains open. |
| V2-22 numeric ranges | Partial | Get-method IDs, seqnos, account transaction fields, block transaction fields, lookup selectors, locate created LT, and shard seqnos now use the upstream signed ranges; the remaining V2 numeric fields are still open. |
| V2-23 getter result stack | Fixed | Both result formats propagate BOC/conversion errors, enforce the upstream depth-100 boundary, and map that boundary to HTTP 533. |
| V2-24 config aliases | Fixed | `getConfigParam` requires exactly one of `param` and `config_id`; both valid aliases and both invalid selector shapes are covered. |
| V2-25 Std zero cursor | Fixed | `getTransactions` treats `lt=0` as absent, while `getTransactionsStd` preserves it as a supplied cursor and returns the canonical empty page for any paired hash. |
| V2-26 archival | Accepted | Localnet retains its complete local account history, so selecting an archival worker has no local meaning. Both flag values follow the same typed and snapshot-covered path. |
| V2-27 transaction lookup | Fixed | All three locate routes share typed address/LT parsing, return the upstream 404 for a miss, deterministically select duplicate tuples, and use the existing outgoing-message index. Real incoming/source transactions plus REST/JSON-RPC errors and boundaries are snapshot-covered. |
| V3-01 transaction ranges/order | Fixed | Transaction hashes are repeatable, time and LT bounds are inclusive, time-filtered requests order by time/LT/account, and other requests use LT/account ordering. Synthetic tie cases and real endpoint boundaries are covered. |
| V3-05 transactions by message | Fixed | The request DTO accepts repeated message hashes, at least one message filter is required with upstream 422, and opcode matching is restricted to inbound messages. Query validation is snapshot-covered and input-only opcode filtering has a focused regression test. |
| V3-06 pending transactions | Partial | Empty and trace-only requests now return upstream 422 because an account filter is required. Account-plus-trace behavior and trace identity remain open. |
| V3-11 jetton wallet sorting | Partial | Explicit balance sorting and filter-dependent ordering follow upstream precedence. Local insertion order approximates database ID order; unclaimed mintless amounts remain unavailable. |
| V3-13 event DTO/order | Partial | Jetton transfer filtering uses the transaction's explicit TL-B `aborted` flag and excludes aborted transfers while retaining aborted burns. All three event routes switch from LT to time ordering when a time bound is present. Trace IDs remain open. |
| V3-14 vesting | Fixed | Empty and combined filters follow upstream AND semantics, including optional whitelist matching. Results are ordered before pagination by first local transaction LT and address. A two-contract stateful test covers empty and non-empty TVM dictionaries, repeated unfiltered reads, combined matches/misses, and page boundaries. |
| V3-15 NFT items | Fixed | Query order follows upstream precedence: insertion/ID order by default, numeric index order for one collection, owner/collection/index order for owner filters, and LT descending when requested. Empty indexes are ignored and repeated collection filters retain multi-filter semantics. Synthetic numeric/null-order cases and two backfilled real items cover pagination. |
| V3-19 information source | Fixed | Both information routes default to the indexed projection and honor `use_v2`; the legacy wallet path reuses the V2 resolver. Missing, uninit, frozen, destroyed, and active non-wallet states are typed and snapshot-covered. Status vocabulary, legacy-only `frozen_hash`, projection-specific 409 behavior, and signed int64 wallet identifiers match upstream. |
| V3-21 trace extras | Fixed | Trace address-book and metadata entries are retained with their trace through filtering and pagination, then merged only for the selected page. A two-trace stateful snapshot covers offset and LT filtering. |
| V3-22 getter result stack | Fixed | Invalid result BOCs and tuple encodings propagate as errors instead of successful empty stacks. |
| CTL-02 force snapshot/import | Partial | Forced replacement retains the existing recovery point until snapshot creation or import succeeds. Imported bytes are still validated only during revert. |

## Rating model

Risk expresses the impact and likelihood of silent incompatibility:

| Rating | Meaning |
|---|---|
| Critical | Can silently return or commit materially wrong chain state, corrupt node state, or break a common typed-client flow. |
| High | Complex projection, filtering, temporal semantics, security, or resource behavior with a plausible silent failure. |
| Medium | Localized status, default, transport, ordering, or uncommon input mismatch. |
| Low | Simple projection or control operation with low blast radius. |

Coverage grades describe the depth of current endpoint tests, independently from the risk rating
and whether those tests assert upstream semantics:

| Grade | Meaning |
|---|---|
| A | Real state transitions plus typed responses and meaningful negative or boundary cases. |
| B | Deterministic integration coverage with some filters, variants, or invalid requests. |
| C | Happy-path, schema, smoke, or single-fixture coverage only. |
| D | Empty/default response or envelope coverage only. |
| O | No endpoint-level coverage found. |

Optional live TonCenter tests are valuable DTO checks, but by themselves do not prove that localnet
filters, ordering, pagination, derived fields, or status codes match upstream.

The grades were assigned from these test layers:

- focused endpoint tests in `tests/localnet/v2_api.rs`, `tests/localnet/v3_api.rs`, and
  `tests/localnet/acton_api.rs`;
- stateful local-node scenarios in `tests/integration/localnet_tests.rs`, including real jetton,
  NFT, sale, multisig, and vesting contracts;
- typed fork-proxy checks in `tests/tonutils-go-contract`;
- optional live DTO tests in `crates/ton-api/tests/toncenter_live_contract`;
- the current LLVM coverage report under `target/coverage/localnet`.

## Executive risk queue

These are the highest-value targets for the next differential test pass:

| Priority | Surface | Why it is dangerous |
|---|---|---|
| P0 | V2 request errors | Axum extractor rejections still bypass the typed envelope. |
| P0 | V3 traces and transactions | Trace identity, trace ranges/ordering, `mc_seqno`, trace summaries, and full transaction DTOs diverge. |
| P0 | V3 message-derived queries | Pending account-plus-trace behavior and inherited trace identity remain incompatible. |
| P0 | V3 token and NFT events | Historical events can disappear after contract changes; trace IDs and parser failure handling remain wrong. |
| P0 | Emulation | Only the root transaction is emulated; downstream cascade, states, cells, address book, and metadata are omitted. |
| P0 | Snapshot load/revert | Applying a snapshot is non-atomic and can leave persistence and memory partially replaced after an error. |
| P1 | V3 deterministic pagination | Several collection endpoints paginate `HashMap`/`HashSet` iteration or use a different upstream sort key. |
| P1 | Streaming | Multi-transaction traces can be emitted repeatedly; WS can replay pre-subscription commits under new filters. |
| P1 | Middleware | Configured `N` RPS becomes an initial burst of `N` followed by 1 RPS; family-specific error bodies are weakly covered. |
| P1 | Source trace | Absolute imports can escape the temporary root and requested compiler versions are not actually selected. |

The inventory contains 105 mounted routes: 19 Critical, 53 High, 21 Medium, and 12 Low. Current
endpoint-test depth is 11 at grade A, 49 at B, 34 at C, 4 at D, and 7 with no endpoint test. The
cross-cutting middleware and fallback rows are not included in these counts.

The current coverage report (`target/coverage/localnet/summary-no-liteapi.json`) reports 87.52% line,
83.65% function, and 83.20% region coverage overall, but records no branch coverage. Important
hotspots are substantially lower:

| Module | Line coverage | Function coverage |
|---|---:|---:|
| V2 JSON-RPC handlers | 70.54% | 55.00% |
| V2 REST handlers | 84.09% | 79.83% |
| V3 handlers | 90.88% | 82.85% |
| Streaming handlers | 79.80% | 89.47% |
| Streaming core | 73.10% | 92.50% |
| Admin handlers | 66.23% | 70.00% |

No branch metric means combinations of optional filters remain poorly evidenced even where line
coverage is high.

## Cross-cutting compatibility risks

| Surface | Risk | Coverage | Finding or missing evidence |
|---|---|---|---|
| V2 GET/POST parity | Accepted | N/A | Upstream accepts GET and POST for most read operations; localnet intentionally exposes only GET for read-only operations. |
| V2 error envelope | High | C | Shared parsers and transaction not-found paths use typed Tonlib envelopes; Axum extraction and unclassified backend errors can still produce incompatible responses. |
| V2 numeric domains | Medium | C | The high-risk signed ranges are covered, but some remaining unsigned local fields can still accept values above upstream `int32`/`int64` schema limits. |
| V3 error status mapping | High | C | Backend failures collapse to 500 and most validation failures use 400; upstream distinguishes 404, 409, and 422. |
| V3 collection pagination | High | C | Multiple handlers scan local maps and paginate before imposing an upstream-compatible stable order. |
| V3 discovery cost | High | C | Several unfiltered endpoints run contract detectors across all accounts; malformed contracts can fail a whole request. |
| V3 live DTO suite | Medium | C | All 35 routes deserialize real TonCenter responses, but semantic equality with localnet is mostly untested. |
| V2 live DTO suite | Medium | C | Seventeen optional live tests cover useful shapes but miss the highest-risk variants. |
| API authentication | Medium | B | Common headers/query paths are covered; malformed/duplicate credentials and WS lifecycle interactions are not. |
| Rate limiting | High | D | Only `limit=1` is tested, which hides the one-token-per-second refill bug. |
| Request recording | Medium | C | Nested V3 routes are collapsed to `jetton`/`nft`; retention and streaming lifetime are untested. |
| Artificial response delay | Medium | C | V2 success is covered; V3/emulate/error responses and concurrent delay updates are not. |
| CORS | Medium | C | Basic preflight is covered; method/header matrix, WS Origin, and auth interaction are not. |
| Compression | Medium | O | JSON/static compression, `Vary`, and explicit SSE exclusion have no endpoint-level tests. |
| Unknown route fallback | High | O | Unknown `/api/*` and `/acton_*` paths fall through to the embedded SPA and can return `200 text/html` instead of an authenticated API 404. |

## TonCenter V2 routes

### Route matrix

| Method and route | Risk | Coverage | Important edge, rare, or complex cases |
|---|---|---|---|
| `POST /api/v2` | Critical | B | Missing/null `id`, `jsonrpc`, or `params`; array params; proxy-equivalent status; every method alias. |
| `POST /api/v2/jsonRPC` | Critical | B | Same envelope matrix; missing `getShards`; REST/RPC output parity. |
| `POST /api/v2/v2/jsonRPC` | Critical | B | Alias behavior and recording/auth parity with the two canonical entry points. |
| `POST /api/v2/sendBoc` | High | B | Empty/malformed/multi-root BOC, replay, duplicate scheduling, canonical 422. |
| `POST /api/v2/sendBocReturnHash` | High | B | Same plus raw hash versus normalized hash and duplicate message behavior. |
| `POST /api/v2/runGetMethod` | High | B | Every stack kind, deep tuple/list, integer boundaries, historical state/libraries, corrupt result stack. |
| `POST /api/v2/runGetMethodStd` | Critical | B | Non-empty canonical Number/Cell/Slice/Tuple/List stacks, invalid shapes and ranges, depth 99/100, exact Std response schema, and live TonCenter DTO coverage. |
| `GET /api/v2/detectAddress` | Low | B | Invalid checksum, testnet/non-bounceable forms, and error status. |
| `GET /api/v2/detectHash` | Medium | B | Padding, wrong length, hex/base64/base64url equivalence, and 422 versus 500. |
| `GET /api/v2/packAddress` | Low | C | Testnet/non-bounceable flags and checksum failures. |
| `GET /api/v2/unpackAddress` | Low | C | Invalid encodings and flags. |
| `GET /api/v2/getAddressInformation` | High | B | Historical `sync_utime` and full-width extra currencies are covered; suspended accounts and fork-history parity remain open. |
| `GET /api/v2/getShardAccountCell` | Medium | A | Exact historical/fork BOC and seqno zero; this C++ checkout has no matching handler, so use a live oracle. |
| `GET /api/v2/getAddressBalance` | Medium | B | Historical and zero seqno and not-found/error transport. |
| `GET /api/v2/getAddressState` | High | C | Nonexistent REST/RPC mismatch, frozen/uninit states, historical and zero seqno. |
| `GET /api/v2/getLibraries` | Low | A | Duplicate/order/large input behavior and a partially invalid list. |
| `GET /api/v2/getExtendedAddressInformation` | Critical | B | Code-less, V3/V4, all pinned Highload V1/V2 and Manual DNS revisions, high-bit IDs/seqnos, missing optional state data, malformed recognized data, raw/friendly address flags, history, and extra currencies are covered; RWallet/PChan states remain raw. |
| `GET /api/v2/getWalletInformation` | Critical | B | Typed real V2-V5 wallets, decoded-code classification, prefix-only/malformed data, stale-hash and highload/non-wallet negatives, exact optional fields, signed seqno, and V5 beta are covered; V1 shares the V1/V2 parser path, while fork-history differential coverage remains open. |
| `GET /api/v2/getTokenData` | Critical | A | Historical seqno, mintless jetton, NFT item/collection, DNS NFT, content variants, stale index, non-token status. |
| `GET /api/v2/getTransactions` | High | A | Exact/unknown cursor, all hash encodings, `to_lt` equality, archival history, decoded messages, extra currencies. |
| `GET /api/v2/getTransactionsStd` | High | A | `lt=0` with hash, exact/unknown cursor, previous transaction boundary, `to_lt` equality, raw message BOCs, and extra currencies. |
| `GET /api/v2/tryLocateTx` | High | A | Real incoming-message lookup, alias parity, decoded text and extra currencies, not-found 404, invalid addresses, and signed created-LT boundaries are covered. |
| `GET /api/v2/tryLocateResultTx` | High | A | Real result lookup, REST/RPC parity, deterministic duplicate selection, and typed validation are covered; fork history remains a separate fixture gap. |
| `GET /api/v2/tryLocateSourceTx` | High | A | Real indexed source lookup, REST/RPC parity, message linkage, not-found 404, and typed validation are covered. |
| `GET /api/v2/getConfigParam` | High | C | Both aliases together, historical mutation, seqno zero, missing param/config cell, status mapping. |
| `GET /api/v2/getConfigAll` | High | C | Historical config mutation, seqno zero, and old/missing blocks. |
| `GET /api/v2/getBlockHeader` | Critical | A | All serialized fields, base/masterchain, single/both hashes, and previous blocks are covered; split/merge/key blocks are outside the current single-shard model. |
| `GET /api/v2/getBlockTransactions` | High | A | Stateful zero/exact/unknown cursors, short-ID mode, masterchain emptiness, one/both hashes, signed ranges, and count bounds. |
| `GET /api/v2/getBlockTransactionsExt` | High | A | Shared cursor and validation behavior, typed full-transaction responses, raw message BOCs, and full-width extra currencies are covered. |
| `GET /api/v2/getMasterchainInfo` | Medium | C | Exact state root/init values, genesis/head zero, and history/reorg. |
| `GET /api/v2/getConsensusBlock` | Medium | C | Server-time semantics, paused/manual mining, virtual time, head zero. |
| `GET /api/v2/getOutMsgQueueSize` | High | D | Real queued messages, correct shard block IDs, configured limit, multiple shards. |
| `GET /api/v2/getShards` | High | B | REST and JSON-RPC typed responses plus proxy-envelope variants; historical descriptors, split shards, and missing blocks remain uncovered. |
| `GET /api/v2/lookupBlock` | Critical | A | Stateful seqno/LT/time lookup plus none/multiple selectors, zero/negative/overflow boundaries, and REST/JSON-RPC parity. |

### V2 findings and accepted deviations

1. **V2-01, `runGetMethodStd` (fixed):** local REST and JSON-RPC used `RunGetMethodRequest` and the legacy
   `[type, value]` decoder. Upstream `RunGetMethodStdHandler` accepts `@type`-discriminated object
   entries and returns the smaller `RunGetMethodStdResult`. The current empty-stack test cannot
   detect either mismatch.
2. **V2-02, JSON-RPC envelope (fixed):** the incoming DTO now requires only `method`, matches
   upstream normalization for absent, null, scalar, empty-array, object, and nonempty-array params,
   and ignores JSON-RPC metadata before dispatch.
3. **V2-03, JSON-RPC methods (fixed):** local dispatch supports upstream `getShards` and rejects
   the non-upstream `shards` name.
4. **V2-04, account status:** JSON-RPC returns `nonexist` while REST maps the same local status to
   V2 `uninitialized`.
5. **V2-05, transport (accepted):** most read routes lack the POST form supported by the C++
   server. This is an intentional localnet simplification and is not scheduled for a fix.
6. **V2-06, errors (partially fixed):** shared request parsers and locate misses now use typed
   Tonlib envelopes; Axum extractor rejections can still bypass them.
7. **V2-07, zero seqno:** account, balance, state, wallet, token, and config handlers accept zero
   and resolve it to the current head; upstream rejects an explicitly supplied zero.
8. **V2-08, token history (fixed):** `getTokenData` detects token data from the account state at
   the requested block instead of reading current indexes.
9. **V2-09, config history (fixed):** historical config requests read the config BOC committed
   with the requested masterchain block.
10. **V2-10, transaction bound (fixed):** local stops before and excludes `lt == to_lt`.
11. **V2-11, transaction cursor (fixed):** all upstream hash encodings are accepted and a
    nonexistent nonzero `(lt, hash)` cursor returns the upstream 500 hash-mismatch error instead of
    neighboring history.
12. **V2-12, transaction DTO (partially fixed):** decoded transaction and locate responses now
    recognize text comments across snake references and expose the upstream legacy `message` and
    decode-error fields. Std/block-ext responses remain raw, and V2/V3 preserve uint32 currency IDs
    plus full VarUint248 amounts. Encrypted/decrypted message classification remains open.
13. **V2-13, extended account state (partially fixed):** code-less states, standard V3/V4,
    Highload V1/V2, and Manual DNS states, revisions, raw/friendly address flags, historical time,
    and extra currencies now match upstream. Recognized malformed state data fails instead of
    becoming a raw success. RWallet and PChan states still fall back to raw.
14. **V2-14, wallet DTO (fixed):** the handler classifies the decoded code BOC and reads the same
    storage prefixes as C++ instead of calling a getter or trusting cached hashes. It emits the
    upstream optional wallet ID/signature fields, signed seqno, and exact V5-beta hash.
15. **V2-15, token DTOs (partially fixed):** mintless claim state and typed DNS NFT data are
    exposed, and NFT collection data is detected from the collection itself. NFT items with a
    collection still lack upstream parent-address verification and collection-derived content.
16. **V2-16, block header (fixed):** every response field is read from the serialized block's
    `BlockInfo`; previous block IDs and `prev_key_block_seqno` no longer use synthetic defaults.
17. **V2-17, block transactions (fixed):** zero/unknown/exact cursors, short transaction mode,
    single versus paired block hashes, signed ranges, and validation statuses match upstream.
    Localnet masterchain blocks contain no account blocks, so an empty masterchain page is correct.
18. **V2-18, block lookup (fixed):** the HTTP contract requires exactly one selector and validates
    seqno, LT, and Unix time in the same signed ranges as the C++ handlers.
19. **V2-19, out queue:** the response is synthetic, always uses the masterchain head, and reports
    zero size/limit instead of per-shard data.
20. **V2-20, consensus timestamp:** local returns the latest masterchain block generation time;
    C++ returns current server time.
21. **V2-21, historical account fields (partially fixed):** `sync_utime` comes from the selected
    block; `suspended` is still always false.
22. **V2-22, numeric ranges (partially fixed):** get-method IDs, seqnos, account/block transaction
    fields, lookup selectors, locate created LT, and shard seqnos use upstream signed ranges. Other
    local unsigned request fields can still accept values that cannot be represented by upstream
    schema types.
23. **V2-23, getter result stack (fixed):** both result formats now reject depth 100 or more with
    code 533 and propagate BOC, tuple, and wire-conversion failures instead of returning a
    successful empty stack.
24. **V2-24, config parameter aliases (fixed):** local handling enforces the same `param` versus
    `config_id` XOR contract as upstream.
25. **V2-25, Std transaction cursor (fixed):** `getTransactionsStd` preserves `lt=0` plus hash as
    a supplied cursor, unlike the intentionally different regular transaction endpoint.
26. **V2-26, archival (accepted):** localnet keeps complete local history and has no separate
    archival worker, so `archival=true` and `false` intentionally select the same data.
27. **V2-27, transaction lookup (fixed):** all locate variants share address and signed-LT
    validation, return the upstream 404 miss, choose duplicate message tuples deterministically,
    and reuse the outgoing-message index for source transactions.

## TonCenter V3 routes

### Route matrix

| Method and route | Risk | Coverage | Important edge, rare, or complex cases |
|---|---|---|---|
| `GET /api/v3/traces` | Critical | B | Page-local extras are covered with offset and LT filtering; add no-filter listing, end-based ranges/order, mc block existence, branched/multi-block identity and summaries. |
| `GET /api/v3/accountStates` | High | B | Missing-row cardinality, contract methods, `code_hash`, frozen details, extra currencies. |
| `GET /api/v3/addressBook` | Medium | C | Mixed valid/invalid batches preserve requested keys with the production null/empty row shape; add DNS names, friendly-address flag variants, and repeated spelling forms. |
| `GET /api/v3/metadata` | High | C | Invalid addresses are ignored in mixed batches; add on/off-chain variants, merge precedence/completeness, and destroyed contracts. |
| `GET /api/v3/addressInformation` | Medium | A | Both `use_v2` projections, their default, and missing/uninit/frozen/destroyed status and `frozen_hash` differences are covered; historical behavior remains. |
| `GET /api/v3/walletInformation` | High | A | Both projections cover missing/uninit/frozen/destroyed and active non-wallet states; signed int64 seqno/wallet IDs are typed. Add every wallet version, malformed data, and optional fields. |
| `GET /api/v3/masterchainInfo` | High | C | Exact block header fields, genesis/head zero and forked history. |
| `GET /api/v3/masterchainBlockShardState` | High | B | Exact header/state values, missing block status, split/merge shard edge cases. |
| `GET /api/v3/masterchainBlockShards` | High | B | Exact headers, empty page, stable ordering and split shard pagination. |
| `GET /api/v3/transactions` | Critical | B | Inclusive boundaries, time-dependent ordering, repeated hash, complete states/phases, rare transaction kinds. |
| `GET /api/v3/messages` | High | B | Header `created_at`, cross-block internal messages, merge direction, nullable external combinations. |
| `GET /api/v3/adjacentTransactions` | High | B | Branch/fanout adjacency, invalid direction, missing result status and inherited transaction DTO. |
| `GET /api/v3/walletStates` | High | B | Signed int64 seqno/wallet IDs are typed; add malformed wallet data, every version, extra currencies, non-wallet and frozen states. |
| `GET /api/v3/topAccountsByBalance` | Medium | C | Ties, deterministic pagination, zero/nonexistent accounts, very large balances and state size. |
| `GET /api/v3/blocks` | High | B | Real header fields, independent selectors, sort ties, split/merge/key blocks. |
| `GET /api/v3/transactionsByMasterchainBlock` | High | B | Missing block status, multi-shard ordering, pagination and inherited transaction DTO. |
| `GET /api/v3/transactionsByMessage` | Critical | B | Required filter, repeated hashes, opcode input-only rule, direction combinations. |
| `GET /api/v3/pendingTransactions` | Critical | C | Required account, trace identity, account-plus-trace semantics, real multi-transaction pending trace. |
| `GET /api/v3/pendingActions` | Low | D | Intentionally empty; validation, pagination envelope, and future transition from the limitation. |
| `GET /api/v3/pendingTraces` | Low | D | Intentionally empty; validation, pagination envelope, and future transition from the limitation. |
| `POST /api/v3/message` | High | B | Invalid BOC/status, duplicate/replay, multi-root input and normalized hash behavior. |
| `POST /api/v3/estimateFee` | High | B | State init, signature checking, bounce/storage/IHR fees and config changes against upstream. |
| `POST /api/v3/runGetMethod` | High | B | Nested stack values, failed exit codes, historical seqno, malformed result stack and libraries. |
| `GET /api/v3/jetton/masters` | High | B | Detection-order pagination and interleaved admin filters are covered; add multiple real masters, metadata variants, and destroyed masters. |
| `GET /api/v3/jetton/wallets` | Critical | B | Balance and filter-dependent ordering, mintless/zero balances, destroyed/frozen/code-upgraded wallets. |
| `GET /api/v3/jetton/transfers` | Critical | B | Aborted tx exclusion, trace ID, destroyed wallet history, time ordering and parser errors. |
| `GET /api/v3/jetton/burns` | High | B | Trace ID, destroyed wallet history, time ordering, malformed bodies and parser errors. |
| `GET /api/v3/nft/items` | High | A | Two backfilled real items cover default, owner, single/repeated collection, empty-index, LT, and page ordering; destroyed items and rare sale types remain. |
| `GET /api/v3/nft/collections` | High | B | Unfiltered creation-order pagination is covered; add multiple real collections, scan cost, metadata variants, and destroyed collections. |
| `GET /api/v3/nft/sales` | High | B | Auctions, telemint/version variants, completed/destroyed sales, pagination and price edge cases. |
| `GET /api/v3/nft/transfers` | Critical | B | Trace ID, destroyed item history, time ordering, bounced/aborted semantics and parser errors. |
| `GET /api/v3/dns/records` | High | C | Length/domain ordering, including equal-length and UTF-8 names, is covered; add real DNS contracts/categories and expired/deleted records. |
| `GET /api/v3/multisig/orders` | High | B | Creation versus update ordering and pagination are covered; add malformed actions, expiry, threshold corners, and multiple real wallets. |
| `GET /api/v3/multisig/wallets` | High | B | Creation versus update ordering and wallet-filtered pagination are covered; add multiple real wallets/orders and malformed config. |
| `GET /api/v3/vesting` | Critical | A | Two real contracts with empty/non-empty whitelists, empty/combined filters, stable ordering, and pagination boundaries; unlock-schedule arithmetic boundaries remain untested. |

### Confirmed V3 findings

1. **V3-01, transaction ranges/order (fixed):** time and LT bounds are inclusive, repeated hashes
   use OR semantics, time-filtered requests sort by time/LT/account, and other requests sort by
   LT/account.
2. **V3-02, trace filters/order:** local requires an identity filter, applies lower ranges to
   `start_*`, matches either start or end mc seqno, and sorts by `start_lt`. Upstream permits no
   identity filter, uses `end_*`, requires completed `mc_seqno_end`, and selects sort keys by range.
3. **V3-03, trace identity:** every local transaction receives its own tx hash as `trace_id` and
   derives an external hash locally. Child transactions must share the root trace identity.
4. **V3-04, trace summary:** local emits an invalid `classification_state="classified"`, computes
   message counts from an incomplete formula, and cannot represent the end block of a multi-block
   trace correctly.
5. **V3-05, transactions by message (fixed):** requests require a message filter, accept repeated
   message hashes, and constrain opcode matching to input messages.
6. **V3-06, pending transactions (partial):** account filters are now required. Local still
   combines account and trace filters differently, and trace matching inherits the
   transaction-hash identity bug.
7. **V3-07, transaction DTO:** standalone transaction states contain hashes but not full state;
   only ordinary details are partially mapped, with credit/bounce and rare transaction kinds lost.
8. **V3-08, message time:** local substitutes enclosing transaction time for message
   `created_at`. An internal message observed in sender and receiver transactions can therefore be
   filtered, ordered, or merged incorrectly.
9. **V3-09, block DTO:** split/merge/key-block, global/version/flags, randomness, validator and
   catchain fields are hardcoded. Selector dependencies are stricter than upstream.
10. **V3-10, account states:** local synthesizes `nonexist` rows upstream does not return, omits
    contract methods, and lacks the upstream `code_hash` filter.
11. **V3-11, jetton wallet sorting (partially fixed):** explicit `sort` orders by balance, while
    absent sorting and address, owner, or single-master filters follow the upstream ordering
    precedence. Local insertion order approximates the upstream database ID; zero-balance filtering
    still cannot include an unclaimed mintless amount that local discovery does not store.
12. **V3-12, historical event loss:** jetton and NFT event handlers rediscover current wallets or
    items before exposing old events. Destroyed, frozen, or code-upgraded contracts can erase
    historical results. Parser errors are silently swallowed with `.ok().flatten()`.
13. **V3-13, event DTO/order (partially fixed):** event parsers now use the explicit ordinary
    transaction `aborted` flag, and jetton transfers exclude aborted transactions while burns keep
    them, matching upstream. Jetton transfer/burn and NFT transfer routes now order by transaction
    time whenever either time bound is supplied, and by LT otherwise. Their responses still omit
    `trace_id`.
14. **V3-14, vesting (fixed):** requests permit no filter or both filters with upstream AND and
    optional whitelist semantics. Empty TVM dictionary values decode as an absent cell, and results
    are deterministically ordered before pagination by first local transaction LT and address.
15. **V3-15, NFT items (fixed):** default pagination follows insertion/ID order; one collection
    sorts by numeric index, owner filters sort by owner/collection/index, repeated or multiple
    collections retain upstream multi-filter ordering, and empty index values are ignored.
16. **V3-16, DNS (fixed):** matching records are ordered before pagination by domain character
    length and then name, matching upstream PostgreSQL text semantics.
17. **V3-17, other collection ordering (fixed):** jetton masters preserve their detection order;
    unfiltered NFT collections and multisig rows use first transaction LT plus address as the
    local stable equivalent of upstream insertion ID. Later contract updates no longer reorder
    multisig pages, and filtered NFT collection queries do not invent an upstream-absent order.
18. **V3-18, address book/metadata invalid input (fixed):** mixed batches retain every requested
    address-book key; invalid keys have null user-friendly/domain fields and empty interfaces,
    matching the production API. Metadata silently ignores invalid addresses.
19. **V3-19, address/wallet information (fixed):** both handlers default to the indexed source and
    honor `use_v2`, with the legacy wallet path reusing the V2 resolver. The two projections use
    the upstream status vocabulary, expose `frozen_hash` only through the legacy address path,
    apply the upstream non-wallet rules to missing, uninit, frozen, destroyed, and active accounts,
    and preserve signed int64 wallet seqno and wallet IDs.
20. **V3-20, errors:** backend errors collapse to 500 and parser errors generally use 400 instead
    of upstream 404/409/422 distinctions.
21. **V3-21, trace extras (fixed):** each mapped trace retains its address book and metadata
    through filtering and pagination; only extras belonging to the selected page are merged into
    the response.
22. **V3-22, getter decoding (fixed):** result BOC and tuple decoding failures propagate as
    errors instead of successful empty stacks.
23. **V3-23, discovery complexity:** contract discovery and several filters scan all accounts and
    run getters repeatedly. A single malformed active contract can fail a whole endpoint, while a
    large state can make cheap-looking queries expensive.

## Emulation and streaming routes

| Method and route | Risk | Coverage | Important edge, rare, or complex cases |
|---|---|---|---|
| `POST /api/emulate/v1/emulateTrace` | Critical | B | Three-hop cascade, bounce/failure, random-dependent code, depth/incomplete traces, downstream metadata and cells. |
| `POST /api/emulate/v1/emulateTonConnect` | Critical | B | Four-message boundary, expired/unit `valid_until`, absent/frozen wallet, insufficient balance, unknown wallet, downstream execution. |
| `POST /api/streaming/v2/sse` | High | C | Trace/token/action events, metadata, filters, duplicate cascades, lag, reconnect and subscription race. |
| `GET /api/streaming/v2/ws` | High | C | Pre-subscribe replay, resubscribe retention, binary/error frames, slow consumers, lag and shutdown close. |

### Emulation and streaming findings

1. **AUX-01, incomplete emulation:** `emulate_trace_by_external_message` executes one external
   message and always returns no children. Upstream builds the complete cascade; local responses
   omit downstream transactions, state changes, code/data cells, address book, and metadata. The
   mapper also hardcodes a zero `rand_seed` and `is_incomplete=false`.
2. **AUX-02, duplicate stream events:** a commit is published per transaction, while each handler
   resolves and emits the complete root trace. A trace of N transactions can produce N copies of
   the same trace and transaction set.
3. **AUX-03, WS subscription history:** the broadcast receiver exists before subscribe but is not
   drained. Accumulated commits can later be processed using a new subscription or resubscription.
4. **AUX-04, WS resubscribe defaults:** omitted optional fields reset locally; upstream preserves
   prior finality/include/action settings.
5. **AUX-05, SSE ID oracle conflict:** the upstream implementation accepts a correlation ID while
   the published SSE schema omits it; local follows the schema and returns null. Resolve which
   contract is authoritative before changing behavior.
6. **AUX-06, invalidation oracle conflict:** the published schema can represent
   `trace_invalidated`, while the upstream implementation rejects it as a subscribable type.
   Local follows the schema; this needs an explicit compatibility decision.
7. **AUX-07, operational stream gap:** SSE and WS silently skip lagged broadcast messages without
   a cursor, replay, or gap notification. This is a reliability risk, not a differential mismatch.

## Localnet control routes

| Method and route | Risk | Coverage | Important edge, rare, or complex cases |
|---|---|---|---|
| `POST /acton_fundAccount` | Medium | B | Amount boundaries, invalid workchain, depleted giver and multi-request races. |
| `GET /acton_getAddressName` | Low | C | Empty/duplicate/invalid addresses and alternate address spellings. |
| `POST /acton_setAddressName` | Low | C | Overwrite/persistence, empty and very long names, duplicate aliases. |
| `GET /acton_getCompilerAbi` | Medium | B | Malformed stored ABI, batch limits and conflicting aliases. |
| `GET /acton_listCompilerAbis` | Low | C | Empty list, deletion and restart persistence. |
| `POST /acton_deleteCompilerAbi` | Medium | O | Missing/existing hashes, aliases, persistence and concurrent lookup. |
| `GET /acton_getVerifiedSource` | High | O | Remote timeout, network failure, malformed upstream response and caching. |
| `GET /acton_getRegisteredVerifiedSource` | Medium | O | Missing/malformed source, restart persistence and hash aliases. |
| `GET /acton_listVerifiedSources` | Medium | O | Empty/large list, ordering, deletion and meaningful `savedAt`. |
| `POST /acton_deleteVerifiedSource` | Medium | O | Missing/existing source, associated ABI lifecycle and persistence. |
| `POST /acton_buildSourceTrace` | High | B | Import escape, compiler parameters/version matrix, dependency cycles and large logs. |
| `POST /acton_registerCompilerAbis` | Medium | C | Multi-entry partial failure, malformed secondary hashes and atomicity. |
| `POST /acton_registerVerifiedSources` | High | O | Partial source/ABI write, malformed bundle, duplicate replacement and rollback. |
| `POST /acton_dumpState` | High | C | Disk errors, large state, concurrent mutation and atomic file replacement. |
| `POST /acton_loadState` | Critical | C | Corrupt/version-mismatched snapshots, missing config/libraries and rollback after apply failure. |
| `POST /acton_snapshot` | High | B | Duplicate with force, failure after deletion, memory limits and concurrent creation. |
| `POST /acton_listSnapshots` | Low | C | Empty/large list and stable ordering. |
| `POST /acton_revert` | Critical | B | Apply failure atomicity, corrupt/inconsistent snapshot and concurrent API calls. |
| `POST /acton_exportSnapshot` | High | C | Write failure, atomic replacement, very large snapshots and invalid destination. |
| `POST /acton_importSnapshot` | High | C | Corrupt/version mismatch, force preservation, deferred validation and large input. |
| `POST /acton_setShardAccount` | High | B | Inconsistent account/history/CAS data, persistence, malformed BOC and hash mismatch. |
| `POST /acton_changeAccountState` | High | B | Deferred freeze is covered; add concurrent mutation and extreme balance/storage values. |
| `POST /acton_sendInternalMessage` | High | C | Malformed BOC, bounce/failure, queue limits, external message and duplicate behavior. |
| `GET /acton_getStartupWallets` | Low | C | Empty/multiple wallets, every wallet version and response consistency after restore. |
| `POST /acton_setNetworkConditions` | Medium | C | Extreme delay, concurrent requests, family matrix and reset during in-flight calls. |
| `POST /acton_setMiningMode` | Medium | O | Mode transitions, in-flight block, idempotence and restart behavior. |
| `POST /acton_mine` | High | B | Unbounded block count, allocation/CPU exhaustion, timestamps and cancellation. |
| `POST /acton_increaseTime` | Medium | C | Overflow, maximum value, repeated updates and interaction with next-block time. |
| `POST /acton_setTime` | Medium | C | Time before latest block, range boundaries and concurrent mining. |
| `POST /acton_setNextBlockTimestamp` | Medium | B | Auto-mining race, overflow and repeated override behavior. |
| `GET /acton_getApiCalls` | Medium | C | Nested V3 route names, retention, streaming lifetime and unauthorized requests. |
| `GET /acton_nodeInfo` | Low | B | Consistency during concurrent mining, restore and network-condition changes. |

### Confirmed control and middleware findings

1. **CTL-01, snapshot atomicity:** `apply_snapshot` replaces persistence and mutates in-memory
   state before final config/library validation. Failure can leave the running node partially
   restored.
2. **CTL-02, force snapshot/import (partially fixed):** an existing snapshot is retained until its
   replacement is successfully built or read. Imported bytes are still not semantically validated
   until a later revert.
3. **CTL-03, mine resource bound:** `blocks: u32` is unbounded and feeds both
   `Vec::with_capacity` and a proportional loop.
4. **CTL-04, source import escape:** bundle entry paths are checked, but absolute/canonicalized
   imports are not constrained to the temporary source root.
5. **CTL-05, compiler version:** requests accept compiler versions at or above the minimum, while
   the implementation always selects one embedded compiler version.
6. **CTL-06, verified source atomicity:** source and compiler ABI registration occur in separate
   write stages and can partially succeed.
7. **CTL-07, rate limit:** `per_second(1).burst_size(limit)` allows an initial burst of `limit` and
   then replenishes one request per second, not `limit` requests per second.
8. **CTL-08, API call names:** nested V3 paths are collapsed to their first segment, including
   `jetton`, `nft`, `dns`, and `multisig`.
9. **CTL-09, OpenAPI drift:** the control OpenAPI omits exposed routes
   `acton_buildSourceTrace`, `acton_getApiCalls`, and `acton_setMiningMode`.
10. **CTL-10, unknown route fallback:** the router's embedded-UI fallback handles unknown API and
    control paths, producing an HTML success response instead of the API family's authenticated
    404/error envelope.

## Differential test backlog

### P0: semantic correctness

1. Send non-empty canonical V2 Std stacks for Number, Cell, Slice, Tuple, and List, including
   nesting depths 99 and 100, and compare both request acceptance and response shape.
2. Exercise raw V2 JSON-RPC with omitted/null envelope fields, empty array/object params,
   `getShards`, and failures that must match the corresponding REST status and envelope.
3. Add suspended-state semantics and specialized RWallet/PChan extended-account fixtures, plus
   parent-verified NFT content to the token fixtures. Highload V1/V2 and Manual DNS fixtures are
   covered.
4. Extend the V2 transaction fixture with encrypted/decrypted bodies and malformed decode cases;
   text/raw bodies, legacy fields, full-width extra currencies, cursors, hash encodings, and LT
   boundaries are covered.
5. Add block-header fixtures if the localnet model gains key blocks, shard splits, or merges;
   ordinary basechain/masterchain fields, pagination cursors, and lookup selectors are covered.
6. Create a branched three-transaction trace spanning at least two blocks. Assert shared trace ID,
   external hash, end block/time/LT, message counts, inclusive ranges, ordering, and page extras.
7. Send one internal message in a later block than its creation. Compare `/messages` filtering and
   ordering around both creation and execution timestamps.
8. Test empty, repeated, and combined filters for transactions-by-message and pending
   transactions; include opcode matches in both message directions.
9. Aborted jetton transfer exclusion and aborted burn preservation are covered. Destroy or change
   wallet/item code and prove old jetton transfer, burn, and NFT transfer events remain queryable
   with correct trace IDs while retaining the upstream ordering rules.
10. Run emulateTrace and emulateTonConnect through an A-to-B-to-C cascade with success, bounce,
    failure, state init, and metadata. Compare every transaction, state delta, and auxiliary cell.
11. Force snapshot restore failures after persistence replacement and after partial memory rebuild;
    prove byte-for-byte rollback of both stores and continued API operation.

### P1: deterministic pagination and operational behavior

1. Create at least two jetton wallets with different balances/LTs and multiple masters/owners;
   assert upstream validation and sort order.
2. Create multiple DNS, distinct NFT collections, jetton-master, collection, and multisig records;
   request every page repeatedly and after unrelated map insertions to detect duplicates, gaps, and
   unstable order. Two backfilled NFT items and two real vesting contracts are covered.
3. Cover rare NFT sale implementations: completed/destroyed fixed-price sales, auctions, telemint,
   and items whose owner/collection changes after their indexed events.
4. Publish one multi-transaction commit and assert exactly one logical streaming notification;
   test pre-subscribe commits, resubscribe field retention, lag and slow consumers.
5. Sustain configured rate limits above 1 RPS across V2, V3, emulate, and streaming; verify refill,
   family-specific errors, auth ordering, and control exemptions.
6. Exercise every currently untested verified-source/control route, import escape attempts, compiler
   versions, partial writes, disk failures, corrupt snapshots, and resource limits.

### P2: transport and rare input parity

1. Build a status/envelope snapshot for every malformed request and not-found case.
2. Exercise signed numeric maxima, values just above upstream maxima, duplicate query parameters,
   empty strings, base64 padding variants, testnet addresses, and invalid checksums.
3. Cover compression, `Vary`, CORS method/header combinations, malformed authentication schemes,
   duplicate WS query tokens, Origin handling, request recording retention, and delayed errors.
4. Test large local state for scan-all V3 endpoints and unbounded control inputs, with explicit
   latency and memory ceilings.

## Exit criteria for this register

An endpoint should be considered compatibility-validated only when tests exercise localnet, use
typed request and response DTOs, cover its meaningful filters and boundary values, and compare
semantic results to the relevant upstream oracle or a fixture derived from that oracle. Coverage
grade A, line coverage, or successful deserialization alone is insufficient.

Every confirmed mismatch should become either:

1. a fix plus a regression test;
2. an explicitly accepted compatibility deviation in the public compatibility document; or
3. a blocked item with the missing state/indexing prerequisite recorded.
