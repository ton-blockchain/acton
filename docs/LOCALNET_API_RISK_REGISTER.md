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

| Finding | Status | Evidence |
|---|---|---|
| V2-01 `runGetMethodStd` | Fixed | Shared `tvm-ffi` Tonlib stack DTOs, separate Std request/result types, local REST/JSON-RPC snapshot, and a live non-empty TonCenter stack test. |
| V2-02 JSON-RPC envelope | Fixed | The incoming proxy DTO accepts method-only requests and upstream params normalization; local and live tests cover ignored metadata and every params shape. |
| V2-03 JSON-RPC `getShards` | Fixed | Dispatch now uses the upstream method name; the non-upstream `shards` name is rejected and local/live typed responses are covered. |
| V2-04 account status | Fixed | REST and JSON-RPC use the same V2 status mapper; a no-state account is covered through both transports. |
| V2-06 validation errors | Partial | Shared V2 validation and JSON-RPC field parsing now return typed 422 envelopes. Axum extractor rejections and locate-miss semantics remain open. |
| V2-07 positive `seqno` | Fixed | All account, shard-account, wallet, token, and config entry points reject explicit zero, negative, and values above signed int32; REST and JSON-RPC are snapshot-covered. |
| V2-09 config history | Fixed | Each masterchain block stores its config hash; config reads and rebuilt historical states use that hash. A real config-param mutation across blocks is covered. |
| V2-22 numeric ranges | Partial | Numeric get-method IDs and `runGetMethodStd.seqno` now use the upstream signed ranges; the remaining V2 numeric fields are still open. |
| V2-23 getter result stack | Fixed | Both result formats propagate BOC/conversion errors, enforce the upstream depth-100 boundary, and map that boundary to HTTP 533. |
| V2-24 config aliases | Fixed | `getConfigParam` requires exactly one of `param` and `config_id`; both valid aliases and both invalid selector shapes are covered. |

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
| P0 | V2 request errors | Axum extractor rejections still bypass the typed envelope, and locate misses can become internal errors. |
| P0 | V2 historical reads | `getTokenData` validates but still ignores a positive historical `seqno` when reading token indexes. |
| P0 | V2 block APIs | Block headers contain many hardcoded fields; block transaction cursors and masterchain transactions are wrong; `lookupBlock` selector rules differ. |
| P0 | V3 traces and transactions | Trace identity, range boundaries, sort keys, `mc_seqno`, trace summaries, and full transaction DTOs diverge. |
| P0 | V3 message-derived queries | `transactionsByMessage` and `pendingTransactions` accept invalid empty filters and use incompatible identity/direction rules. |
| P0 | V3 token and NFT events | Historical events can disappear after contract changes; trace IDs, abort filtering, ordering, and parser failure handling are wrong. |
| P0 | Emulation | Only the root transaction is emulated; downstream cascade, states, cells, address book, and metadata are omitted. |
| P0 | Snapshot load/revert | Applying a snapshot is non-atomic and can leave persistence and memory partially replaced after an error. |
| P1 | V3 deterministic pagination | Several collection endpoints paginate `HashMap`/`HashSet` iteration or use a different upstream sort key. |
| P1 | Streaming | Multi-transaction traces can be emitted repeatedly; WS can replay pre-subscription commits under new filters. |
| P1 | Middleware | Configured `N` RPS becomes an initial burst of `N` followed by 1 RPS; family-specific error bodies are weakly covered. |
| P1 | Source trace | Absolute imports can escape the temporary root and requested compiler versions are not actually selected. |

The inventory contains 105 mounted routes: 19 Critical, 53 High, 21 Medium, and 12 Low. Current
endpoint-test depth is 2 at grade A, 53 at B, 39 at C, 4 at D, and 7 with no endpoint test. The
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
| V2 error envelope | High | C | Parse errors, Axum extraction errors, backend errors, and not-found results produce 400/500/plain responses instead of upstream 404/422 Tonlib envelopes. |
| V2 numeric domains | Medium | D | Local unsigned fields accept values above upstream signed `int32`/`int64` ranges. |
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
| `GET /api/v2/getAddressInformation` | High | B | Historical `seqno`/`sync_utime`, zero seqno, frozen state, extra currencies, suspended accounts. |
| `GET /api/v2/getShardAccountCell` | Medium | A | Exact historical/fork BOC and seqno zero; this C++ checkout has no matching handler, so use a live oracle. |
| `GET /api/v2/getAddressBalance` | Medium | B | Historical and zero seqno and not-found/error transport. |
| `GET /api/v2/getAddressState` | High | C | Nonexistent REST/RPC mismatch, frozen/uninit states, historical and zero seqno. |
| `GET /api/v2/getLibraries` | Low | A | Duplicate/order/large input behavior and a partially invalid list. |
| `GET /api/v2/getExtendedAddressInformation` | High | C | Uninit/frozen, specialized wallet/DNS/RWallet/PChan states, revision, history, extra currencies. |
| `GET /api/v2/getWalletInformation` | High | C | V3/V4 wallet ID, V5 signature flag, all revisions, malformed data, getter failure, historical state. |
| `GET /api/v2/getTokenData` | Critical | C | Historical seqno, mintless jetton, NFT item/collection, DNS NFT, content variants, stale index, non-token status. |
| `GET /api/v2/getTransactions` | High | C | Exact/unknown cursor, all hash encodings, `to_lt` equality, archival history, decoded messages, extra currencies. |
| `GET /api/v2/getTransactionsStd` | High | C | `lt=0` with hash, exact/unknown cursor, previous transaction boundary, `to_lt` equality. |
| `GET /api/v2/tryLocateTx` | High | C | Not-found 404, ambiguous identical tuples, and created-LT boundaries. |
| `GET /api/v2/tryLocateResultTx` | High | C | Not-found 404, ambiguity, fork/archive lookup. |
| `GET /api/v2/tryLocateSourceTx` | High | C | Not-found 404, ambiguity and full-scan cost, fork behavior. |
| `GET /api/v2/getConfigParam` | High | C | Both aliases together, historical mutation, seqno zero, missing param/config cell, status mapping. |
| `GET /api/v2/getConfigAll` | High | C | Historical config mutation, seqno zero, and old/missing blocks. |
| `GET /api/v2/getBlockHeader` | Critical | C | Every header field, base/masterchain, split/merge/key block, hashes and previous blocks. |
| `GET /api/v2/getBlockTransactions` | High | B | Initial zero cursor, unknown cursor, masterchain tx, one/both hashes, and count bounds. |
| `GET /api/v2/getBlockTransactionsExt` | High | B | Same plus full message and extra-currency DTOs. |
| `GET /api/v2/getMasterchainInfo` | Medium | C | Exact state root/init values, genesis/head zero, and history/reorg. |
| `GET /api/v2/getConsensusBlock` | Medium | C | Server-time semantics, paused/manual mining, virtual time, head zero. |
| `GET /api/v2/getOutMsgQueueSize` | High | D | Real queued messages, correct shard block IDs, configured limit, multiple shards. |
| `GET /api/v2/getShards` | High | B | REST and JSON-RPC typed responses plus proxy-envelope variants; historical descriptors, split shards, and missing blocks remain uncovered. |
| `GET /api/v2/lookupBlock` | Critical | C | LT/time selectors, none/multiple selectors, boundaries, seqno zero, shard/workchain validation. |

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
6. **V2-06, errors:** invalid requests commonly become 400/500 or Axum text instead of a 422
   Tonlib envelope; locate misses that are 404 upstream can become 500.
7. **V2-07, zero seqno:** account, balance, state, wallet, token, and config handlers accept zero
   and resolve it to the current head; upstream rejects an explicitly supplied zero.
8. **V2-08, token history:** `getTokenData` ignores `seqno` and reads current indexes.
9. **V2-09, config history:** historical config requests validate a block but still read the
   current config BOC.
10. **V2-10, transaction bound:** local includes `lt == to_lt`; upstream stops at and excludes it.
11. **V2-11, transaction cursor:** only standard base64 is accepted locally, while upstream also
    accepts padded base64, base64url, and hex. Whether a nonexistent `(lt, hash)` cursor must fail
    instead of returning neighboring history remains a high-confidence suspicion needing a fixture.
12. **V2-12, transaction DTO:** message bodies are always raw and extra currencies are empty;
    upstream can return decoded text/encrypted/decrypted fields and decode errors.
13. **V2-13, extended account state:** the absence of specialized wallet/DNS/RWallet/PChan states
    is confirmed. The exact uninitialized/frozen variant mismatch remains high-confidence pending a
    differential fixture.
14. **V2-14, wallet DTO:** `wallet_id` and V5 `is_signature_allowed` are absent; getter errors are
    swallowed into `seqno: null`.
15. **V2-15, token DTOs:** mintless claim state and DNS NFT data are absent; NFT collection
    `next_item_index` is inferred from item metadata; content classification is too narrow.
16. **V2-16, block header:** many semantic fields are hardcoded and `prev_key_block_seqno` is
    populated from the immediate previous block.
17. **V2-17, block transactions:** the canonical zero cursor returns an empty page, individual
    root/file hash handling differs, and masterchain blocks always return no transactions.
18. **V2-18, block lookup:** local accepts zero or multiple selectors and silently prioritizes
    seqno, LT, then time; upstream HTTP validation requires exactly one selector.
19. **V2-19, out queue:** the response is synthetic, always uses the masterchain head, and reports
    zero size/limit instead of per-shard data.
20. **V2-20, consensus timestamp:** local returns the latest masterchain block generation time;
    C++ returns current server time.
21. **V2-21, historical account fields:** `sync_utime` remains current node time and `suspended` is
    always false.
22. **V2-22, numeric ranges (partially fixed):** numeric get-method IDs and the Std getter seqno now
    use upstream signed ranges. Other local unsigned request fields can still accept values that
    cannot be represented by upstream signed schema types.
23. **V2-23, getter result stack (fixed):** both result formats now reject depth 100 or more with
    code 533 and propagate BOC, tuple, and wire-conversion failures instead of returning a
    successful empty stack.
24. **V2-24, config parameter aliases:** upstream requires exactly one of `param` and `config_id`;
    local handling does not enforce the same XOR contract.
25. **V2-25, Std transaction cursor:** upstream treats `lt=0` plus a hash as a supplied cursor;
    local validation rejects or interprets that boundary differently.
26. **V2-26, archival:** transaction requests parse the upstream `archival` option but local
    execution ignores it.

## TonCenter V3 routes

### Route matrix

| Method and route | Risk | Coverage | Important edge, rare, or complex cases |
|---|---|---|---|
| `GET /api/v3/traces` | Critical | B | No-filter listing, end-based ranges/order, mc block existence, branched/multi-block identity and summaries. |
| `GET /api/v3/accountStates` | High | B | Missing-row cardinality, contract methods, `code_hash`, frozen details, extra currencies. |
| `GET /api/v3/addressBook` | Medium | C | Invalid input preservation, DNS names, interface variants, repeated spelling forms. |
| `GET /api/v3/metadata` | High | C | On/off-chain variants, merge precedence/completeness, invalid addresses, destroyed contracts. |
| `GET /api/v3/addressInformation` | Medium | B | `use_v2`, frozen/nonexistent sources, status parity and historical behavior. |
| `GET /api/v3/walletInformation` | High | B | Active non-wallet 409, all wallet versions, malformed data and optional fields. |
| `GET /api/v3/masterchainInfo` | High | C | Exact block header fields, genesis/head zero and forked history. |
| `GET /api/v3/masterchainBlockShardState` | High | B | Exact header/state values, missing block status, split/merge shard edge cases. |
| `GET /api/v3/masterchainBlockShards` | High | B | Exact headers, empty page, stable ordering and split shard pagination. |
| `GET /api/v3/transactions` | Critical | B | Inclusive boundaries, time-dependent ordering, repeated hash, complete states/phases, rare transaction kinds. |
| `GET /api/v3/messages` | High | B | Header `created_at`, cross-block internal messages, merge direction, nullable external combinations. |
| `GET /api/v3/adjacentTransactions` | High | B | Branch/fanout adjacency, invalid direction, missing result status and inherited transaction DTO. |
| `GET /api/v3/walletStates` | High | B | Malformed wallet data, every version, extra currencies, non-wallet and frozen states. |
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
| `GET /api/v3/jetton/masters` | High | B | Stable unfiltered pagination, deterministic admin, metadata variants and destroyed masters. |
| `GET /api/v3/jetton/wallets` | Critical | B | Balance sorting, multi-master validation, mintless/zero balances, destroyed/frozen/code-upgraded wallets. |
| `GET /api/v3/jetton/transfers` | Critical | B | Aborted tx exclusion, trace ID, destroyed wallet history, time ordering and parser errors. |
| `GET /api/v3/jetton/burns` | High | B | Trace ID, destroyed wallet history, time ordering, malformed bodies and parser errors. |
| `GET /api/v3/nft/items` | High | B | Stable default ordering, multi-collection validation, destroyed items and rare sale types. |
| `GET /api/v3/nft/collections` | High | B | Upstream order, scan cost, metadata variants, destroyed collections and stable pagination. |
| `GET /api/v3/nft/sales` | High | B | Auctions, telemint/version variants, completed/destroyed sales, pagination and price edge cases. |
| `GET /api/v3/nft/transfers` | Critical | B | Trace ID, destroyed item history, time ordering, bounced/aborted semantics and parser errors. |
| `GET /api/v3/dns/records` | High | D | Real DNS contracts/categories, deterministic length/domain order, expired/deleted records. |
| `GET /api/v3/multisig/orders` | High | B | ID versus LT ordering, malformed actions, expiry, threshold corners and multiple wallets. |
| `GET /api/v3/multisig/wallets` | High | B | Stable pagination, multiple wallets/orders, include-orders behavior and malformed config. |
| `GET /api/v3/vesting` | Critical | B | Empty/combined filters, deterministic pagination, multiple contracts and boundary unlock schedules. |

### Confirmed V3 findings

1. **V3-01, transaction ranges/order:** local time bounds are exclusive and sorting is always by
   LT/hash. Upstream bounds are inclusive and time-filtered requests sort by time, LT, account.
   Repeated `hash` parameters are not represented locally.
2. **V3-02, trace filters/order:** local requires an identity filter, applies lower ranges to
   `start_*`, matches either start or end mc seqno, and sorts by `start_lt`. Upstream permits no
   identity filter, uses `end_*`, requires completed `mc_seqno_end`, and selects sort keys by range.
3. **V3-03, trace identity:** every local transaction receives its own tx hash as `trace_id` and
   derives an external hash locally. Child transactions must share the root trace identity.
4. **V3-04, trace summary:** local emits an invalid `classification_state="classified"`, computes
   message counts from an incomplete formula, and cannot represent the end block of a multi-block
   trace correctly.
5. **V3-05, transactions by message:** an empty filter returns general transactions instead of
   422; message hash is singular; opcode filtering can apply to output messages although upstream
   constrains it to input messages.
6. **V3-06, pending transactions:** local permits empty and trace-only queries and combines
   account/trace differently; upstream requires account filters. Trace matching also inherits the
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
11. **V3-11, jetton wallet sorting:** `sort` orders by last transaction LT locally but by balance
    upstream. Required validation for multiple masters plus owner is missing.
12. **V3-12, historical event loss:** jetton and NFT event handlers rediscover current wallets or
    items before exposing old events. Destroyed, frozen, or code-upgraded contracts can erase
    historical results. Parser errors are silently swallowed with `.ok().flatten()`.
13. **V3-13, event DTO/order:** jetton transfer/burn and NFT transfer responses omit `trace_id` and
    ignore upstream time-based ordering. Only jetton transfers incorrectly include aborted txs.
14. **V3-14, vesting:** local requires exactly one contract/wallet filter; upstream permits none or
    both. Pagination traverses an unordered set instead of upstream ID order.
15. **V3-15, NFT items:** default pagination follows map iteration and multi-collection validation
    is absent.
16. **V3-16, DNS:** default result order is unstable; upstream orders by domain length and name.
17. **V3-17, other collection ordering:** jetton masters, NFT collections, and multisig routes use
    unstable or different pagination keys from the upstream database ID order.
18. **V3-18, address book/metadata invalid input:** local rejects the whole request. Upstream keeps
    invalid address-book keys with null data and silently ignores invalid metadata addresses.
19. **V3-19, address/wallet information:** both handlers parse and ignore `use_v2`.
    `walletInformation` additionally returns 200 with empty wallet fields for an active non-wallet
    instead of upstream 409.
20. **V3-20, errors:** backend errors collapse to 500 and parser errors generally use 400 instead
    of upstream 404/409/422 distinctions.
21. **V3-21, trace extras:** address book and metadata are collected before filtering and
    pagination, so a page can leak extras for traces absent from that page.
22. **V3-22, getter decoding:** a result stack decode failure is converted to successful empty
    stack rather than an error.
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
2. **CTL-02, force snapshot/import:** an existing snapshot is deleted before the replacement
   snapshot is successfully built or read. Imported bytes are not semantically validated until a
   later revert.
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
3. Mutate an account, token, and config across blocks; query each historical seqno plus zero and
   prove state, config, token metadata, `sync_utime`, and validation semantics.
4. Build V2 transaction fixtures for exact and invalid cursors, all hash encodings,
   `to_lt == tx.lt`, decoded text bodies, extra currencies, and masterchain block transactions.
5. Compare every V2/V3 block-header field on basechain, masterchain, key, split, and merge blocks;
   cover zero and unknown pagination cursors and all lookup selector combinations.
6. Create a branched three-transaction trace spanning at least two blocks. Assert shared trace ID,
   external hash, end block/time/LT, message counts, inclusive ranges, ordering, and page extras.
7. Send one internal message in a later block than its creation. Compare `/messages` filtering and
   ordering around both creation and execution timestamps.
8. Test empty, repeated, and combined filters for transactions-by-message and pending
   transactions; include opcode matches in both message directions.
9. Execute an aborted jetton transfer, then destroy or change the wallet code. Historical transfer
   must remain queryable with correct trace ID and ordering. Repeat for burn and NFT transfer.
10. Run emulateTrace and emulateTonConnect through an A-to-B-to-C cascade with success, bounce,
    failure, state init, and metadata. Compare every transaction, state delta, and auxiliary cell.
11. Force snapshot restore failures after persistence replacement and after partial memory rebuild;
    prove byte-for-byte rollback of both stores and continued API operation.

### P1: deterministic pagination and operational behavior

1. Create at least two jetton wallets with different balances/LTs and multiple masters/owners;
   assert upstream validation and sort order.
2. Create multiple vesting, DNS, NFT, jetton-master, collection, and multisig records; request every
   page repeatedly and after unrelated map insertions to detect duplicates, gaps, and unstable order.
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
