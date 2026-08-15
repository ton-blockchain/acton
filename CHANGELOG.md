# Changelog

All notable changes to this project will be documented in this file.

## [Unreleased]

This development cycle expands Acton from a contract CLI into a broader TON
development platform. It adds Acton Studio, a full local TON network, Actonscan,
source verification, a testnet faucet, a native language server, substantially
more capable RPC and localnet tooling, and a shared explorer and transaction UI.

`acton localnet` remains the lightweight in-process simulator. Acton Studio can
manage both simulator environments and full local TON networks; those full
networks are powered by the separate Localton runtime. The new verifier service,
API, and web UI are included below, while the `acton verify --new` CLI
integration remains hidden and preview-only.

### Breaking Changes and Migration

- Human-readable currency terminology changed from TON/nanoton to
  GRAM/nanogram across the CLI, debugger, reports, APIs, UI, documentation, and
  standard library. Serialized action failures now use
  `not_enough_grams_to_send` and `cannot_reserve_grams`. `ton(...)`,
  `reserveToncoinsOnBalance(...)`, and the `{:ton}` formatter remain
  compatibility aliases. Faucet deployments should replace
  `FAUCET_AMOUNT_NANOTONS`, `ANTIFRAUD_WALLET_BALANCE_MAX_NANOTONS`, and
  `ANTIFRAUD_SENT_AMOUNT_WINDOW_MAX_NANOTONS` with their `*_NANOGRAMS`
  equivalents. The old names and `TON` suffix remain accepted, but the faucet
  `/stats` field is now `total_sent_nanograms`.
- `crypto.getFastRandomBytes` no longer accepts a seed. Set deterministic state
  first with `testing.setRandomSeed(seed)` or `random.setSeed(seed)`, then call
  `crypto.getFastRandomBytes(bytes)`.
- Regenerate generated wrappers. `fromStorage(...)` now accepts an optional
  workchain, deployments preserve workchain and shard settings, and generated
  contracts expose the new `toShard` state used for non-basechain deployment.
- Actonscan is now the default explorer for links printed by `acton script`.
- Custom networks default to TON global id `-3` for Wallet V5 derivation. This
  can change Wallet V5 addresses on custom networks that previously inherited
  mainnet-like behavior; set `networks.<name>.global-id = -239` for a
  mainnet-compatible endpoint.
- `ExternalSendResult.isAccepted()` now fails with an actionable diagnostic
  after a real-network broadcast. Broadcast only confirms submission and cannot
  prove destination acceptance; use `waitForFirstTransaction()` or
  `waitForTrace()`. The result exposes the distinction through
  `acceptanceKnown`.
- Local, non-fork test and script worlds now initialize `testing.getNow()` and
  VM time from the wall clock. Forks use the selected block timestamp. Tests
  that require deterministic time must call `testing.setNow(...)`.

### CLI, RPC, Build, and Wrappers

- Added `acton rpc call <address> <method> [args...]` for typed get-method calls
  on mainnet, testnet, localnet, and custom networks. It accepts ABI method names
  or numeric ids, parses Tolk values, decodes results, supports raw TonCenter
  stacks, and emits structured JSON errors.
- `acton rpc call` and `acton rpc info` gained `--block-number` for pinned
  masterchain state and `--abi <path>` for explicit compiler ABI JSON or Tolk
  interface files. `rpc call --with-comments` includes ABI field descriptions
  in both human and JSON output.
- RPC inspection can identify Jetton masters and wallets, multisig wallets, and
  other catalogued contracts. It decodes complex and union storage, any-address
  values, Jetton metadata and balances, multisig thresholds and participants,
  custom errors, and nested typed results.
- `rpc info`, `rpc call`, `rpc trace`, and `doc abi` now fall back to verified
  ABI data when the project and bundled catalog do not contain a match. Results
  are cached under `build/cache/verifier-abi`, with stale-cache fallback during
  verifier outages.
- Added `acton doc abi <contract-or-code-hash>` for formatted compiler ABI JSON
  from project contracts, the built-in ABI catalog, or the verifier.
- `acton build --output-sources <DIR>` and `[build].output-sources` now emit
  `<contract>.source.json` registration artifacts with source, ABI, code hash,
  compiler, and debug metadata. Precompiled BoC contracts do not emit source
  bundles.
- Per-contract Tolk and TypeScript wrapper settings are available under
  `[contracts.<name>.wrappers.*]`. Fields inherit independently from project
  defaults, while CLI flags keep highest priority.
- `acton test` accepts multiple files and directories in one invocation and
  deduplicates overlapping selections across normal, coverage, profiling, gas,
  and mutation runs.
- Added `acton test --no-capture` for live test stdout and stderr while still
  retaining captured output for JUnit and Test UI.
- Script execution now reports nested `runGetMethod` assertion failures with a
  failing process status, ignores pre-script transactions while waiting, uses
  the script ABI for message decoding, and provides clearer missing-library,
  broadcast, network, and toolchain diagnostics.
- Acton HTTP clients now send an `acton/<version>` user agent. The bundled ABI
  catalog and executor network configuration received repeated data updates.

### Testing, Emulation, Debugging, and Profiling

- Added source-level gas profiles through `--gas-profile`,
  `--gas-profile-format cpuprofile|collapsed`, and
  `--gas-profile-include-tests`. Test UI renders contract and per-test
  flamegraphs with source locations, self and total gas, and instruction tables.
- Added `testing/bench`. `bench.measure(...)` profiles a zero-argument callback
  and returns the result, total gas, per-instruction statistics, and TASM;
  `bench.format` and `bench.formatDiff` render profiles and comparisons.
- Added deterministic test controls through `testing.setRandomSeed(seed?)` and
  `testing.setChecksigIgnore(bool)`.
- Added `ExternalSendResult.hideTraceFromUi()` and
  `SendResultList.hideTraceFromUi()` for excluding setup traces from saved
  bundles and Test UI without changing test-visible transactions.
- Added inclusive `toBeInRange`, `lisp_list<T>` containment, emptiness, and
  length matchers, plus broader transaction, external-message, rollback,
  backtrace, fuzzing, snapshot, and mutation coverage.
- Forked tests, scripts, and localnet now resolve account state, blockchain
  configuration, libraries, time, and block context from one pinned
  masterchain snapshot. Per-seqno account, library, and configuration caches are
  reused across runs and can fall back to stale data during provider outages.
- Remote contracts that reference on-chain libraries now trigger automatic
  discovery and registration in forked tests, scripts, localnet, RPC, and
  source/ABI lookup.
- Get-method emulation now exposes the account's actual balance. Nonexistent or
  uninitialized localnet accounts return exit code `-13`, and local execution
  honors current time and network configuration more consistently.
- Debug and retrace views gained Tolk source stepping, TASM instructions, stack
  and cell inspectors, source locations, debug values, exit-code lenses, trace
  selection, and browser-side source tracing through the source-trace WASM
  package.
- Test UI gained decoded storage, storage diffs, end balances, trace-wide value
  flow, gas and fee summaries, transaction actions, contract display names,
  treasury-trace collapsing, richer failure diagnostics, and broad Playwright
  visual coverage.

### Standard Library, Formatter, Linter, and Compiler

- Updated the bundled compiler to Tolk v1.4.2 and adopted `grams(...)`,
  `reserveGramsOnBalance(...)`, `{:gram}`, and `{:grams}` terminology.
- Added `crypto.rawSignSlice(privateKey, data)` for byte-aligned Ed25519 slice
  signing compatible with `CHKSIGNS` and `isSliceSignatureValid`.
- Added `boc.encode<T>` and `boc.decode` for indexed or non-indexed, optional
  CRC32C BoCs containing cells or unread slice data.
- Extended `format(...)` and `println(...)` with binary output, prefixed
  hexadecimal, fixed width, custom fill, alignment, and sign- or prefix-aware
  zero padding that ignores ANSI escape sequences.
- Dynamic pack/unpack and rendering now cover `bitsN`, bit-string map keys,
  `addr_none`, maps, generic containers, large numeric values, and legacy empty
  TonCenter list values more consistently.
- Added linter rule `E031`, `unnecessary-not-null-assertion`, with a safe
  automatic fix. Compiler and linter diagnostics are also available through the
  language server.
- `tolkfmt` now preserves comments placed between annotations and declarations.
- Fixed the emulation config map (`BlockchainConfigMap`) to key parameters by a
  signed 32-bit id, matching TON's `Hashmap 32 ^Cell = ConfigParams`. This lets
  `setParamRaw`/`getParamRaw` address out-of-consensus **negative** config
  params (for example `-137`) instead of throwing a range-check error, and the
  values round-trip with `blockchain.configParam` (`CONFIGOPTPARAM`).

### Acton Studio

- Added Acton Studio, started with `acton studio start`. It defaults to
  `127.0.0.1:3015`, opens the browser, enforces one instance per project, and
  currently rejects non-loopback hosts because environment control and project
  wallet signing are unauthenticated.
- Studio discovers the selected project, contracts, source artifacts, ABIs, and
  supported V4R2 and V5R1 wallets. It can deploy contracts and route project
  mainnet, testnet, simulator, and full-network requests through persistent
  virtual environments.
- Added stored test runs with suites, logs, traces, coverage, gas profiles, and
  mutation events. `acton test` reports this data to a matching Studio instance
  by default; `--no-studio-reporting` and `[test].studio-reporting = false`
  disable it. Reporting is opportunistic and does not fail tests when Studio is
  unavailable.
- Added persistent environment lifecycle management, startup progress, staged
  error reporting, project locking, API-call capture, generated OpenAPI, source
  and contract registries, deployment, wallet lists, and environment-specific
  endpoints. Captured calls are available at
  `GET /api/v1/environments/{environment_id}/api-calls` for every environment.
- Full Local TON environments can import selected addresses, bootstrap the V3
  indexer, expose account actions, and manage cold snapshots. New environments
  use the test-only chain id `-3`. Settings remain visible while an environment
  is stopped.
- Studio gained explorer, tests, contracts, simulator tools, debugger, faucet,
  wallets, snapshots, API reference, configuration, control API, settings, and
  troubleshooting pages, plus a complete documentation section.

### Localnet Simulator

- Added interval and manual block production with `--block-interval-ms`,
  `--no-mining`, `acton localnet mine`, `/acton_mine`, and optional empty-block
  mining. Submitted messages are queued into blocks, and automatic mining runs
  only while messages are pending unless empty-block mining is enabled.
  Generated blocks include a simplified masterchain, state updates, Merkle
  updates, proofs, and previous-masterchain-block TVM context.
- Added virtual-time commands and APIs for increasing time, setting current
  time, or selecting the next block timestamp.
- Added optional API authentication through `--require-auth`, bearer tokens,
  `X-API-Key`, and WebSocket query tokens. Static UI assets remain public.
- Added persistent SQLite state through `[localnet].db-path`, state file
  dump/load through `acton localnet state`, and named in-memory checkpoints
  through `acton localnet checkpoint`, including import and export over HTTP.
  Imports validate histories, hashes, references, transactions, messages,
  queues, and configuration before atomic replacement.
- Added typed TonCenter v2, v3, Emulation, and Streaming APIs with stable error
  models, browser CORS, SSE and WebSocket subscriptions, paged histories, rate
  limiting, configurable latency, and API-call timing. Metadata lookup supports
  batched address names and compiler ABIs, and get methods on nonexistent or
  uninitialized accounts return exit code `-13`.
- Expanded TonCenter compatibility across blocks, messages, transactions,
  accounts, wallet and token data, masterchain and shard state, fee estimation,
  Jetton and NFT activity, DNS, multisig, vesting, pending data, address books,
  metadata, and Ton Connect emulation.
- Added an optional LiteServer-compatible binary API for blocks, accounts,
  transactions, configuration, libraries, message submission, and get methods.
  Enable it with `--liteapi`; its default port is the HTTP port plus one, and
  `--liteapi-port` selects another port. Proof-dependent methods remain limited
  and require no-proof client mode.
- Added hard block limits for serialized size, gas, and logical time. Deferred
  messages and freezes remain queued for later blocks instead of creating
  unbounded blocks.
- Added Jetton funding, account-state changes, startup account naming, forked
  ABI registration, verified-source integration, library-backed contracts,
  system elector messages, and tick-tock transactions. Startup accounts are
  exposed through `acton_getStartupAccounts`.
- Improved persistence, block size, API latency, old-state reuse, transaction
  pagination, Jetton and NFT indexing, metadata, bounce handling, and graceful
  streaming shutdown.

### Full Local TON and Indexing

- Added Localton, a persistent isolated TON network with a genesis validator,
  DHT server, LiteServer, keys, zerostate, wallet management, validators,
  election participation, staking, rewards, hardfork configuration, raw
  LiteServer commands, and optional external messages.
- Localton supports wallet V1, V2, V3, V4R2, V5R1, and Highload V2; node and
  validator management; configuration, admin, faucet, API v2, and LiteServer
  endpoints; and cold snapshot create, list, restore, and delete operations.
- Added a full Docker Compose stack with validators, API v2 and v3, PostgreSQL,
  Redis, a V3 worker, action classifier, and generated OpenAPI documents.
- Added `ton-indexer-core` and `ton-indexer-liteserver` for validated canonical
  masterchain and shard batches, idempotent storage, durable checkpoints, and
  at-least-once delivery.
- Contract indexing validates Jetton wallets, NFT collections, and multisig
  relationships; resolves DNS and off-chain metadata through bounded,
  deduplicated caches; supplies global libraries; and discovers conventional
  get-method dictionaries.
- Added the Actonscan backend with direct LiteServer indexing, persisted TPS
  windows, checkpoints, all-time opcode statistics, example transaction hashes,
  generated OpenAPI, and an opcode-to-ABI coverage task.

### Actonscan and Explorer

- Added standalone Actonscan for mainnet, testnet, and custom
  TonCenter-compatible networks. Custom networks can be edited, persisted, and
  shared without API keys; address formatting, history, names, and branding are
  network-aware.
- Added account, transaction, action, block, token, Jetton, NFT, Locker,
  Vesting, Multisig, suspended-contract, verified-source, ABI catalog,
  favorites, configuration, Cell Inspector, Emulate, faucet, and statistics
  pages.
- Explorer routes preserve selected networks, source files, blocks, catalog
  pages, and transaction-tree nodes. Search covers addresses, DNS and Telegram
  names, blocks, transactions, bundled registry names, favorites, and local
  names.
- Added a generated, network-specific address registry merged from public and
  Acton-maintained sources, weekly update automation, multi-source name
  tooltips, and JSON import/export for favorites and local names.
- Block pages now show masterchain and shard relationships, full block ids,
  time navigation, global version and capabilities, fees, transaction fallback
  loading, historical network configuration, validators, bridges, suspended
  addresses, oracle values, and known system-contract metadata.
- Transaction inspection combines message flow, action overviews, value flow,
  transaction trees, state changes, raw details, source, TASM, retracing, and
  cell inspection. Stable contract letters connect decoded addresses to tree
  nodes.
- Large traces now use previews, lazy body decoding, incremental branch loads,
  memoized panels, deferred node details, and visible-action name resolution.
  Partial traces reconstruct the causal path and clearly mark omitted segments.
- Added Cell Inspector for Base64, hexadecimal, `ton://`, and explorer-link BoCs
  with multiple roots, compiler ABI or custom TL-B parsing, canonical block
  schemas, disassembly, raw cells, verified source, saved drafts, and exotic
  cell fallback.
- Added Emulate for ABI-built or raw internal and external messages, account
  overrides, time and signature settings, enriched traces, state changes,
  debugging, localnet submission, editing an existing message, and 30-day
  shareable emulations.
- Added token, NFT, wallet, vesting, locker, and multisig-specific views;
  minting, DNS renew, approval, signer, schedule, order, safety, collection, and
  holder experiences; and an NSFW registry using anonymized hashes.
- Added favorites for accounts, blocks, and transactions; exact bigint GRAM and
  token formatting; streaming account updates; historical navigation; lazy
  account and block loading; copy actions; QR and address variants; Open Graph
  images; installable-app metadata; and responsive layouts.
- Added edge-cached historical TonCenter proxies for mainnet and testnet and a
  network TPS panel backed by the Actonscan backend.
- Repeated ABI catalog updates added hundreds of opcode and code-hash mappings,
  substantially reducing unknown messages in sampled traces.

### Source Verifier

- Added the verifier service, API, and web UI for Tolk, FunC, and Tact. The
  service recompiles submitted source, compares its code hash with an address
  or supplied hash, and stores an immutable source bundle in a Git-backed
  registry. The separate `acton verify --new` CLI integration remains hidden
  and preview-only; it is not the default verification flow.
- The restricted Node.js compiler worker supports multiple compiler versions,
  generated registry loaders, import mappings, compiler ABI data, Tolk source
  maps, Tact package metadata, generated files, and Tact-to-Tolk ABI conversion.
- Tact ABI conversion reconstructs omitted storage, deployment and system-cell
  metadata, contract parameters, maps, references, integer formats, and
  compiler-allocated continuation cells.
- Added verification, status, source, ABI, recent-verification, statistics,
  history, OpenAPI, health, and version endpoints. Lookups accept addresses,
  hexadecimal, and standard or URL-safe Base64 code hashes.
- Source bundles contain compiler metadata, entrypoint, verified timestamp,
  storage revision, source-bundle hash, manifest hash, source files, ABI, and
  optional source maps. Source lookup returns one nullable `bundle`, and the
  same single-bundle shape is used by generated source artifacts and localnet
  source registration. Repeated verification can return `already_verified`
  with a nullable `compiled_code_hash`. Registry commits use auditable
  code-hash subjects and metadata.
- Verifier source paths are limited to 128 portable ASCII characters, compared
  case-insensitively for duplicates, and reject traversal, Git control paths,
  multiple source extensions, invalid language extensions, and other
  non-portable forms.
- The source registry can rebuild its SQLite index from Git, supports
  configurable storage roots and independent commit/push controls, preserves
  exact source bytes, rejects dirty repositories at startup, and supports
  API-key-protected historical timestamp imports.
- Compiler execution clears inherited environment variables, limits file
  access through the Node permission model, disables writes and unsafe runtime
  features, and patches older Emscripten compilers for the restricted loader.
- Added the shared verifier UI and Actonscan verified-contract catalog with
  source browsing, downloads, compiler links, ABI and source-map views,
  pagination, statistics, charts, Open Graph images, and local Explorer links.
- The hidden `acton verify --new` flow uploads normalized multipart bundles,
  validates optional deployed addresses, retries transient failures, and treats
  `already_verified` as success.

### Testnet Faucet

- Added the Rust testnet faucet service with proof-of-work challenge and claim
  endpoints, SQLite-backed jobs, one wallet worker, TonCenter retries, Valkey
  windows, structured logs, graceful shutdown, health, version, statistics,
  OpenAPI, Docker, and deployment workflows.
- Challenges are bounded, expiring, versioned, atomically consumed, tied to the
  normalized destination and device, and provide solve-time and nonce limits
  honored by `acton wallet airdrop` and Actonscan.
- Added layered antifraud for destination balances, global sent amounts,
  successful claim counts, devices, authenticated GitHub identities, guest IPs,
  and client subnets, plus a dynamic expiring blacklist and trusted-proxy rules.
- Added GitHub App authentication with device-bound sessions and configurable
  verified or established account tiers based on age, repositories, and
  followers.
- Added the Actonscan Testnet Faucet page with browser/WASM proof-of-work,
  address and usage history, testnet validation, GitHub connection, recoverable
  redirects and sessions, and links to the equivalent CLI workflow.

### Language Server and Editor Tooling

- Added a native and WASM-capable language server for Tolk, Fift, TASM, TL-B,
  and `Acton.toml`, shipped through `acton ls` and the official VS Code
  extension.
- Tolk support includes semantic highlighting, type- and context-aware
  completion, auto-imports, postfix and snippet completion, definitions, type
  definitions, references, workspace symbols, rename, file-rename import edits,
  highlights, folding, formatting, hover, inlay hints, signature help, code
  actions, call hierarchy, and selection ranges.
- Fift support includes completion, navigation, hover documentation, semantic
  highlighting, code lenses, folding, and gas hints. TASM supports instruction
  completion, hover, code lenses, and folding. TL-B supports declarations,
  references, completion, symbols, semantic highlighting, hover, and tag/type
  hints.
- `acton ls` supports stdio or one TCP client, configurable logging, standard
  library paths, profiling through `ton/profile`, and disassembly through
  `ton/disassemble`. It reports the Acton version during initialization.
- Added schema-backed runtime settings for completion, auto-imports,
  references, hints, compiler/linter diagnostics, and language-specific
  features. Settings update open-document diagnostics dynamically.
- The language server publishes Tolk compiler errors and Acton linter findings
  with quick fixes, registers workspace watchers, handles non-ASCII positions,
  normalizes import mappings, and starts even when `.acton/tolk-stdlib` is not
  present.
- `Acton.toml` support resolves file references, documents linter rules, and
  shows the installed Acton version for `toolchain.acton`. The VS Code extension
  provides actionable setup errors, multiline script execution, and Actonscan
  links by default.

### Developer and Contributor Changes

- Building Acton from source now requires Rust 1.96.1.
- Repository contributors now need Git LFS to check out documentation images
  and visual snapshots. Users of official release binaries are not affected.
- Repository-local frontend packages moved from `crates/` and `ui-e2e/` into
  `packages/`. `@acton/localnet-ui` was replaced by `@acton/studio-ui` and
  reusable `@acton/explorer-core`; transaction rendering moved into its own
  package; `ton-indexer` was renamed to `ton-indexer-contracts`; and the Rust
  source-map crate was renamed to `tolk-source-map`.
- Browser-side TASM consumers moved from `ton-assembly` to `@ton/tasm`, and the
  frontend toolchain moved to TypeScript 7.0. Repository integrations that
  import the old package or depend on the old TypeScript configuration must
  update.
- The legacy `ton-ls` executable was replaced by the native multi-language
  server started through `acton ls`. Third-party editor integrations must move
  to the standard LSP methods and the documented `tolk.getTypeAtPosition`,
  `ton/profile`, and `ton.disassemble` extensions. The official VS Code
  extension moved to `packages/vscode-ton` and now uses this server.
- Added incremental Tolk project resolution and cached type inference with
  stable file ids, importer invalidation, source providers, speculative file
  databases, cyclic return inference, generic narrowing, overload handling, and
  receiver-type reuse.
- Added reusable analysis for constants and enums, method ids, struct opcodes,
  serialization sizes, control-flow graphs, read/write/mutation facts, active
  parameters, declaration ownership, modifiers, and source ranges.
- Added Tree-sitter FunC and TOML grammars, typed FunC syntax, initial FunC
  project indexing, scopes, cross-file resolution, navigation, and basic type
  inference. Grammar builds now support `wasm32-unknown-unknown`.
- Added a documented shared UI system and gallery with tables, pagination,
  dialogs, popovers, tooltips, themes, code viewers, parsed values, technical
  chips, exact GRAM and token amounts, dates, durations, paths, hashes, opcodes,
  source locations, disclosures, inputs, skeletons, loaders, toasts, and
  transaction-specific components.
- Explorer, Studio, Test UI, verifier, and transaction views were migrated to
  the shared packages, improving accessibility, responsive behavior, consistent
  copy actions, exact formatting, loading states, and theme startup.
- JavaScript and CSS linting moved from ESLint to Biome. Bun installs use
  hardened configuration, pinned versions, minimum release ages, and `bun ci`.
  Project templates gained safer dependency-install defaults.
- Added or expanded OSV scanning, dependency audits, release attestations,
  application and Docker checks, source-trace and grammar WASM builds, address
  registry updates, label automation, and path-aware CI for Studio, Actonscan,
  Localton, verifier, faucet, UI, templates, and VS Code.
- Updated vulnerable Rust, JavaScript, template, docs, Tree-sitter, and compiler
  dependencies and refreshed generated locks and snapshots without assigning
  unsupported vulnerability claims to dependency-only commits.
- Application project layouts, Docker metadata, toolchains, deny policies,
  release workflows, crate metadata, generated assets, and dependency rules
  were aligned across Actonscan, Localton, verifier, faucet, and the workspace.

### Documentation and Distribution

- Documentation was expanded for Studio, localnet, LiteAPI, state and
  checkpoints, RPC, gas profiling, verifier, testing, source artifacts,
  wrappers, standard-library availability, agent skills, IDEs, deployment, and
  the new UI tools. Preview deployments now opt out of search indexing.
- Embedded Studio and Test UI assets are precompressed with gzip. Project
  templates are packed into one deterministic Zstandard archive, and bundled
  TVM instruction data is compressed and loaded lazily to reduce binary size.

## [1.1.0] - 22.05.2026

Acton 1.1.0 is the first feature release after `v1.0.0`. It focuses on
external-message testing, transaction matcher precision, BoC contract support,
library publishing safeguards, formatter/debugger ergonomics, Tolk 1.4.1
update, and a broad documentation refresh.

### Breaking Changes and Migration

- Regenerate generated wrappers after upgrading. External incoming message
  wrapper methods now return `ExternalSendResult` from `net.sendExternal(...)`,
  so checked-in Tolk wrappers should be refreshed with `acton wrapper <contract>`
  to expose accepted/rejected metadata and the new external-in helpers and
  matchers.

### CLI and Tooling

- `acton fmt` now supports `--stdin`, allowing editor integrations to format
  Tolk source from standard input without creating a temporary file. Invalid
  `--stdin`/path combinations now fail with normal clap diagnostics.
- `acton script` now reports missing generated dependency helpers with a more
  direct error. Missing files such as `@gen/JettonWallet.code.tolk` are
  highlighted in the error output, and script execution stops immediately after
  the build failure.
- `[test].ui` in `Acton.toml` is now applied correctly as if `--ui` had been
  passed on the command line.
- `acton up` now correctly supports the bare `trunk` version alias.
- Generated command docs, man pages, and help snapshots were refreshed around
  formatter, testing, trace, verification, wrapper, localnet, and retrace
  behavior.

### Libraries

- `acton library publish` now checks local `libraries.toml` and global
  `global.libraries.toml` for an exact hash and network match before publishing.
  When a match exists, Acton warns that top-up is usually cheaper than creating a
  new masterchain library account.
- In interactive mode, publishing a tracked library can top up an existing local
  or global library entry instead of publishing a duplicate. If several tracked
  entries match, Acton lets the user choose the exact entry and updates that
  entry's `last_topup_timestamp` after a successful top-up.
- Before sending a publish transaction, Acton now performs a warning-only
  on-chain library lookup by hash. If the library already exists on-chain, Acton
  prints a warning and still continues publication. Lookup errors are ignored so
  the publish flow is not blocked by temporary RPC failures.

### Build, Wrappers, and Contracts

- Added initial support for contracts represented by an explicit BoC plus an
  ABI-like Tolk type file. Build, wrapper generation, script execution, RPC
  info, and tests now cover runtime code loaded from BoC sources.
- `acton wrapper --all` now handles BoC contracts consistently: it includes BoC
  contracts with valid type metadata and skips or reports actionable errors for
  BoC contracts without usable type information.
- Template scripts and wrappers were cleaned up for Jetton, NFT, and W5
  extension projects, including common helper updates and regenerated wrapper
  output.
- Template project READMEs now describe generated project structure and
  validation commands more consistently across contract-only and app templates.

### Testing, Matchers, and Emulation

- Added first-class test-runner support for external-in messages.
  `net.sendExternal(...)` now returns `ExternalSendResult`, a wrapper that keeps
  the produced `SendResultList` when the message is accepted and preserves
  emulator failure metadata when the external-in message is rejected before an
  accepted transaction trace is produced.
- `ExternalSendResult` exposes the accepted trace through `transactions` and the
  rejected-send metadata through `error`, including the emulator message,
  whether the message was not accepted, the VM exit code when available, and an
  internal diagnostic id used by Acton to retrieve richer emulator details.
- Added external-in helpers for tests:
  `ExternalSendResult.isAccepted()`, `unwrap()`, `at()`, `giveName()`,
  `waitForFirstTransaction()`, `waitForTrace()`, and
  `findExternalOutMessage()`.
- Added external-in matchers:
  `expect(result).toBeAccepted()`,
  `expect(result).toBeNotAccepted()`, and
  `expect(result).toHaveExternalVmExitCode(...)`. Failed checks now render the
  external status, emulator reason, compute/action failure details, known TVM
  exit-code descriptions or ABI error names, source location, and
  `onExternalMessage` backtrace details with `--backtrace full`.
- External-in diagnostics now cover rejected messages without
  `acceptExternalMessage()`, missing or invalid account state, mismatched
  `StateInit`, missing libraries, explicit VM throws before acceptance, accepted
  compute/action failures, and external send failures before any transaction
  trace is produced.
- Transaction search parameters now support `sendMode`, so
  `findTransaction(...)`, `toHaveTx(...)`, `toNotHaveTx(...)`,
  `toHaveSuccessfulTx(...)`, `toHaveFailedTx(...)`, and `executeTill(...)` can
  filter child transactions by the parent `SEND_MODE_*` action that produced
  them. Mismatch diagnostics render expected send modes as named constants.
- Local emulation now honors Param 45 precompiled contract entries by contract
  code hash for transaction execution and get-method C7 state, matching the
  fixed gas and zero-step transaction shape used by the network.
- Gas profiling and transaction formatting now resolve message names from both
  destination and source contract ABIs, improving diagnostics when the message
  was sent to an unexpected contract.
- Coverage output tables were tightened for more compact CLI display.
- Snapshot write failures now fail tests instead of being silently ignored.
- `acton test --save-test-trace` and Test UI trace metadata now better preserve
  contract display names separately from stable contract ids/names, including
  path-like display names.
- Trace saving and UI reporting now log missing emulation results to test
  stderr, making trace/report mismatches easier to diagnose.

### Debugging, Runtime Rendering, and Test UI

- Debugger rendering now treats Tolk `map<K, V>` values as first-class `MapKV`
  values instead of generic structs. Empty maps display as `{}`, non-empty maps
  show their entry count in DAP variable summaries, and map entries still expand
  as child variables.
- Empty extra-currency maps in the runtime `InMessage.valueExtra` field now
  display as `map<int32, varuint32> = {}` instead of a confusing `()` or raw
  empty cell.
- The debugger no longer recompiles all project contracts when resolving the
  known treasury code hash, avoiding unnecessary work for treasury frames.
- Debugger expression evaluation keeps map entries accessible through field
  paths, including backticked numeric keys.
- Debugger stepping and TxCursor support were improved, including better
  filtering for step-in and new integration coverage for cursor-based traces.
- Test UI now uses contract display names from trace metadata when available
  while keeping stable contract ids for file lookup.
- Contract metadata files in saved traces now include richer display-name
  information, making UI labels more accurate for generated or path-like
  contract names.
- Potential trace-loading, missing-emulation, and contract-metadata issues now
  have clearer diagnostics in the UI/backend path instead of silently producing
  `0 transactions`.

### Standard Library, Formatter, and Linter

- Updated the bundled Tolk compiler to Tolk v1.4.1.
- Added `parseCellFromBase64` to the standard library.
- Improved TON and nanoton formatting for large values and edge cases.
- Numeric diff output now better explains mismatches between different integer
  types.
- Linter rule documentation and implementation were refreshed for compiler
  errors, imports in contracts, bounce handlers, naming rules, documented throw
  values, and unauthorized access.
- Tolk linter internals gained consistency fixes around per-root settings and
  rule diagnostics.

### Documentation and Website

- The CI setup page was rewritten and corrected for GitHub Actions and GitLab
  CI, including frontend validation guidance and clearer Acton setup examples.
- Testing documentation was expanded across built-in matchers, coverage, gas
  profiling, trace bundles, configuration, fork testing, cookbook examples, and
  custom matchers.
- Build-system, wrapper, CLI command, linter, standard-library, scripting,
  deployment, verification, project-management, walkthrough, and tutorial pages
  received broad factual and wording improvements.
- The docs site gained OS-specific install tabs, no-copy controls for selected
  snippets, richer file-tree rendering, Mermaid diagrams, gas-report
  highlighting, Acton CLI grammar improvements, dotted Tolk annotation
  highlighting, and an `Acton.toml` file icon.
- Landing and installation pages were refreshed with updated assets, corrected
  universal links, dynamic `tonconnect-manifest.json`, better light-theme
  styling, fixed play-button styling, corrected redirects, and updated Open
  Graph metadata.

### Localnet Preview

> Warning: the localnet features listed in this section are still preview work
> and are not available to end users yet. They are documented here so the
> release notes capture the repository changes, but they should not be treated
> as a stable or supported 1.1.0 feature surface.

- Added localnet status reporting and initial admin/control state endpoints.
- Renamed localnet control/admin endpoints to the `acton_*` namespace and added
  `acton_setShardAccount`, `acton_sendInternalMessage`, state dump/load, and
  snapshot-oriented flows.
- Added an initial OpenAPI description for the localnet control API and docs
  generation support for that API.
- Localnet now uses `127.0.0.1` instead of `localhost` for generated endpoints.
- Internal messages sent through TonCenter-compatible endpoints are now
  rejected, with raw internal-message flows moved to the Acton-specific
  endpoint.
- Added work-in-progress wallet support, faucet simplification, explorer
  dashboard pages, search, wallet and token/NFT views, trace transaction APIs,
  and improved fork-mode badges in the localnet UI.
- Added localnet support for `@ton/ton` stack formatting and USDT-like jettons
  with library wallets and off-chain metadata.
- Fixed localnet emulation endpoint behavior and simplified obsolete state
  source endpoints by removing `acton_setStateSource` and
  `acton_getStateSource`.

## [1.0.0] - 11.05.2026

Acton 1.0 is the first stable release available to everyone. It marks the
result of six months of work, thousands of engineering hours, and hundreds of
thousands of lines of code.

Acton rethinks smart-contract development on TON: fast tests, straightforward
testnet and mainnet deployment, local and production debugging, AI-assisted
workflows, and many other tools that finally make smart-contract development
productive and approachable.

Learn more about Acton on the official website:

https://ton-blockchain.github.io/acton/

## [0.5.0] - 10.05.2026

Acton 0.5.0 is a focused public-release follow-up to 0.4.0, adding TON Connect
support for verification approval transactions, improving typed cell and
cell-tree formatting, making mutation testing and coverage work for dependent
contracts, expanding wrapper generation and starter templates, refreshing the
Tolk compiler and TON executor config, and tightening documentation, release
CI, debugger snapshots, and UI inspection flows.

### Breaking Changes and Migration

- `acton up` now reads release metadata only from the public
  `ton-blockchain/acton` repository. The temporary fallback repository used
  during the public-release transition is no longer queried, so mirrors or
  tooling that depended on fallback release metadata should switch to the
  primary repository.
- Compiler ABI JSON now uses `client_ty_idx` on struct fields that have
  `@abi.clientType(...)` after the Tolk compiler update. Direct ABI consumers
  should read the indexed client type from `unique_types` instead of relying on
  the previous field shape.
- The Acton linter no longer ships the `E023`
  `incoming-messages-duplicate-opcode` rule because duplicate incoming-message
  opcodes are now handled by the Tolk compiler. Configurations that explicitly
  enable, disable, or explain `E023` should remove that rule reference.

### CLI, Wallets, and Verification

- Added `acton verify --tonconnect` and `--tonconnect-port` so contract
  verification can be approved through a TON Connect wallet instead of a stored
  Acton wallet.
- Added `acton library publish --tonconnect` and
  `acton library topup --tonconnect`, with `--tonconnect-port`, so library
  publication and top-up transactions can also be approved through TON Connect.
- `acton up` now targets the public Acton release repository, keeps a hidden
  `--yes` flag for JetBrains plugin compatibility, and reports release lookup
  and release-list failures with clearer GitHub/network diagnostics.
- Wallet airdrops now use the new faucet endpoint and wait for airdrop
  completion more reliably.
- Wallet airdrop challenge handling now validates the challenge version before
  using the response.
- Wallet airdrop challenge requests now use the faucet's JSON `POST` flow with
  the target address and TON airdrop type, matching the current faucet API.
- `acton script` now gives a clearer error when `waitForTrace()` cannot find a
  trace, including the timeout path used by scripts that print the result.
- `acton verify`, `acton up`, and related generated man/help output were
  regenerated around the new flags and release repository behavior.

### Project Templates and Wrappers

- Tolk wrapper generation now supports external incoming messages, including
  contracts that expose both internal and external message surfaces.
- The bundled Tolk compiler/TON objects and TypeScript wrapper generator were
  updated to the Tolk 1.4.
- Starter templates and app scaffolds were normalized across Counter, Empty,
  Jetton, NFT, and W5 Extension projects: app templates gained `.env.example`
  files, generated project metadata became more consistent, and the empty-app
  and W5 app templates now include project-specific `AGENTS.md` guidance.
- Generated GitHub Actions workflows in starter templates are now split into
  contract and dApp checks where appropriate, cover both `main` and `master`,
  use least-privilege permissions and concurrency cancellation, and pin the
  refreshed `setup-acton` action.
- App template `npm run test` scripts now succeed without requiring an Acton
  project, so generated dApp-only workflows can run independently from contract
  checks.
- `acton new --templates` now returns richer machine-readable template
  metadata, and generated help/man output was refreshed around the updated
  template list and app scaffolds.
- Generated contract headers now use the local Git user name when available,
  falling back to `Acton User` when it is missing.
- Jetton, NFT, and W5 Extension templates received consistency fixes, including
  unified author metadata, kebab-case NFT script names, refreshed W5 wrapper
  helpers, and regenerated TypeScript wrappers.
- The W5 Extension starter template was finalized with refreshed message
  definitions, generated Tolk wrappers, and TypeScript wrappers.
- Template opcode and hex literal casing is now normalized to lowercase in the
  built-in templates.

### Stdlib, Formatting, and ABI Decoding

- Added the `{:cell-tree}` formatter for `format()` and `println()` so
  cells, slices, builders, and typed `Cell<T>` values can be rendered as a tree
  of cell references.
- Typed `Cell<T>` values now display decoded data when the compiler ABI can
  parse the cell, improving `println`, formatted output, and debugger/type
  views.
- `Expectation<SendResultList>.toEmitExternalMessage<T>()` now reports a much
  more actionable failure, including the searched message type/opcode and the
  transaction list context.
- Exit-code formatting now distinguishes compute-phase and action-phase exit
  codes, so known codes are shown in the correct phase-specific context.
- Small opcodes such as `0x1` are formatted more consistently in transaction
  and message output.
- `tolk-fmt` handles `!` chains more predictably and no longer breaks
  single-argument generic type syntax such as `<T>` in common chains.

### Testing, Mutation, Coverage, and Debugging

- Mutation testing now supports contracts that depend on the mutated contract,
  including embedded and library-ref dependencies such as Jetton minter/wallet
  setups. Dependent contracts are rebuilt with the mutated dependency override
  before child test runs.
- Targeted `acton build <contract>` runs now refresh generated dependency-code
  helpers for parent contracts that embed or reference the rebuilt contract,
  preventing stale `library_ref` and embedded-code helper files.
- Test trace snapshot paths now normalize test names, which makes generated
  trace artifacts more stable and filesystem-friendly.
- Debug rendering now prints empty cells, slices, and builders as explicit
  `empty cell`, `empty slice`, and `empty builder` values, and storage decoding
  is more reliable in debugger snapshots.
- Coverage now works for library-reference-based contracts such as Wallet W5.
- Coverage now also resolves project contracts deployed from generated
  dependency-code helpers such as `gen/*.code.tolk`, which fixes coverage for
  dependent-contract flows like Jetton minter/wallet setups.
- W5 debugging no longer emits an unnecessary warning and handles the W5 flow
  correctly.

### UI and Trace Inspection

- Test UI now warns when the connection to the runner is lost.
- Parsed cell/slice views can parse values even when remaining bits are present,
  which is useful for W5 and other partially decoded payloads.
- Parsed cell, slice, and builder values now include a button for copying the
  full hex BoC.
- UI packages were updated alongside the compiler ABI refresh and typed-cell
  decoding changes.
- Shared UI transaction rendering was refined for the updated tutorial and
  inspection flows, including clearer account details, disassembly, action
  summaries, transaction tree entries, and exit-code chips.

### Docs, Release CI, and Internal Polish

- Documentation gained wallet-management, verification, and deployment how-to
  guides, a refreshed quickstart/walkthrough, a full tutorial flow, agent-skills
  pages, and style corrections across Acton.toml, debugging, testing, IDE
  support, installation, libraries, and welcome pages.
- JetBrains and VS Code documentation was expanded with reorganized screenshot
  assets, new extension feature coverage, terminal/action/test-runner views, and
  updated demo media.
- Documentation gained new dApp development and project-management guides,
  including TypeScript wrapper workflows and expanded library, scripting,
  walkthrough, and IDE-support coverage.
- The documentation site now redirects `/docs` to `/docs/welcome` locally and
  supports richer file-tree visualization in docs pages.
- Documentation gained a reusable `Callout` component and stricter external
  link validation around redirects.
- Release CI now generates cargo-dist manifest checksums for release binaries,
  links the released `acton-installer.sh`, and removes obsolete mirroring
  workflows for trunk, objects, and release artifacts.
- Documentation deployment and labeler workflows now skip draft pull requests.
- Dockerfile links were updated to match the current public-release layout.
- `ton-objs` archive checksum mismatch diagnostics now mention
  `TON_OBJS_DISABLE_ARCHIVE_SHA_VERIFY` for environments that intentionally
  bypass archive verification.
- Tree-sitter dependencies were refreshed, including the `ip-address` update,
  and the TON executor config was updated.

## [0.4.2] - 08.05.2026

Test release.

## [0.4.1] - 08.05.2026

Test release with public repository.

## [0.4.0] - 04.05.2026

Acton 0.4.0 is a broad follow-up to 0.3.0. It stabilizes the dApp and wrapper
surface, adds project-level toolchain pinning, expands RPC, retrace, debugger,
Test UI, and localnet inspection workflows, and tightens testing, coverage,
linter, formatter, docs, templates, and editor integrations.

### Breaking Changes and Migration

- The short `-v` verbosity flag was removed from `acton test`,
  `acton script`, and `acton retrace`. Use the long `--verbose` flag for
  executor logs and retrace detail output.

  ```bash
  # before
  acton test -v
  acton script scripts/deploy.tolk -v
  
  # after
  acton test --verbose
  acton script scripts/deploy.tolk --verbose
  ```

  Root `acton -v` remains the version shortcut, so downstream wrappers should
  avoid assuming that `-v` means command-local verbosity.

- `SendResultList.wait()` was renamed to
  `SendResultList.waitForFirstTransaction()` and now returns `SendResult?`
  instead of `bool`. This makes the confirmed on-chain root transaction
  available to scripts instead of only reporting whether it was found.

  ```tolk
  // before
  val ok = txs.wait();
  if (!ok) {
      return;
  }
  
  // after
  val applied = txs.waitForFirstTransaction();
  if (applied == null) {
      return;
  }
  println("applied at lt {}", applied!.lt);
  ```

- `acton build` now writes Tolk compiler ABI JSON files to `build/abi/` by
  default. If custom tooling reads ABI artifacts from the main build output
  directory, update it to read `build/abi/<contract>.json` or pin the old-style
  location explicitly:

  ```toml
  [build]
  output-abi = "build"
  ```

  The CLI override is `acton build --output-abi <DIR>`.

- The TypeScript wrapper directory spelling was normalized from
  `wrapper-ts/` to `wrappers-ts/`. Update `Acton.toml`,
  frontend imports, generated-project checks, and documentation snippets that
  still reference the singular form.

  ```toml
  # before
  [wrappers.typescript]
  output-dir = "app/src/wrapper-ts"
  
  # after
  [wrappers.typescript]
  output-dir = "app/src/wrappers-ts"
  ```

- Acton HTTP clients now ignore `HTTP_PROXY`, `HTTPS_PROXY`, `ALL_PROXY`, and
  system proxy settings by default to avoid macOS sandbox proxy autodetection
  crashes. Set `ACTON_USE_PROXY=1` or `ACTON_USE_PROXY=true` when your
  environment requires those proxy settings.

- Several linter diagnostics were reclassified from error-style `E...` codes
  to style `S...` codes, and the linter docs were regenerated around the new
  numbering. Update `Acton.toml` lint configuration, CI filters, snapshot
  expectations, and inline suppressions if they reference numeric rule codes
  directly.

- Compiler ABI JSON now stores canonical types in `unique_types` and references
  them by `ty_idx`, including monomorphic struct and alias instantiations.
  Tooling that reads ABI JSON directly should stop expecting inline `ty`,
  `target_ty`, `body_ty`, `return_ty`, or `prefix_str` fields and use the new
  indexed fields instead. The reflection helpers
  `reflect.typeAbiJsonOf*()` were replaced with `reflect.typeUniqueIdxOf*()`.

### CLI, Project, and Network Workflows

- Added project-level toolchain pinning through `[toolchain] acton = "..."`
  in `Acton.toml`. Project commands check the configured Acton version before
  running, while `acton up` remains available from the same directory so users
  can install the expected version.
- New projects and templates now pin the current Acton version, include clearer
  `.env.example` guidance, and document proxy and TonCenter API-key behavior
  more explicitly.
- Added `acton init --stdlib-only` for refreshing `.acton/` without reading or
  patching `Acton.toml`.
- `acton wallet list` no longer requires an `Acton.toml`, which makes wallet
  inventory commands usable outside an initialized project.
- `acton script` now supports TON Connect flows, ABI-driven argument parsing,
  clearer trailing argument forwarding with `--`, better non-interactive
  wallet errors, and remote-state cache invalidation after broadcasting.
- Testnet wallet airdrop requests now include a stable non-empty
  `x-device-uid` header derived from the local machine identifier, while
  keeping the device value out of the JSON claim payload.
- Added `acton rpc trace` for rendering TonCenter v3 traces as stable decoded
  transaction trees, plus `acton rpc block` and `acton rpc block-number` for
  latest masterchain block inspection.
- Added Nushell support to `acton completions`, while the completion generator
  and root help now share the same base command metadata, including version
  flag aliases.
- Remote account-state loading now uses `/api/v2/getShardAccountCell` where
  available, and localnet implements the same endpoint for compatibility with
  the emulator and tracing stack.
- `acton compile` now exits with code `1` for missing files and reports
  conflicting stdout/file output choices more clearly.
- Acton HTTP calls now send a versioned `acton/<version>` `User-Agent` across
  update, doctor, wallet, verifier, localnet, and API-client workflows.
- `acton up` no longer special-cases Homebrew-style installation paths.
- CLI help, command descriptions, color handling, wallet setup hints, and
  non-interactive `acton new` errors were tightened across the command surface.

### Wrappers, Templates, and dApps

- Added Vite-based dApp scaffolding through `acton init --create-dapp`,
  including standalone empty-app support and generated TypeScript wrapper usage
  in Counter, Jetton, NFT, and wallet-extension flows.
- Added the Wallet W5 Extension template and aligned it with the Counter,
  Jetton, NFT, and empty-app template families.
- Templates now share more app components and styles, use Acton's TON Connect
  manifest, display full traces in scripts, pin Acton versions, and include
  stronger README, script, ESLint, wrapper, and byte-for-byte consistency
  checks.
- App templates now inject normalized npm package names while preserving
  `package.json` and `package-lock.json` field order, avoiding unnecessary
  lockfile churn in generated projects.
- Counter, Jetton, NFT, empty-app, and wallet-extension templates were refined
  with owner checks, cleaner tests, better TonCenter key handling, Tolk
  metadata strings, fewer unnecessary casts, and more consistent generated app
  wiring.
- Jetton, NFT, and wallet-extension template tests now use more consistent
  import grouping, helper placement, and `test <domain>:` name prefixes, and
  Jetton scripts now point their default metadata image at the Acton logo
  instead of the broken TON symbol URL.
- Added `acton wrapper --all` for regenerating wrappers across configured
  contracts.
- Generated wrappers now integrate typed `@abi.clientType(...)` declarations,
  use `.gen` filenames, avoid unused imports, include explicit return types,
  and no longer suppress formatter and linter checks by default.
- TypeScript wrapper generation was updated and template commands now expose a
  clearer wrapper-regeneration path for projects that include generated app
  code.

### Testing, Coverage, and Stdlib

- Added `@test.skip("description")`. Skip and TODO reasons now appear in
  console output, Test UI, JUnit, and TeamCity reports.
- `acton test` now fails when no tests are selected, validates custom networks
  earlier, reports missing wallets with better setup guidance, and correctly
  merges CLI flags with `Acton.toml` settings.
- Fork-mode tests now preserve remote last-transaction LT/hash metadata from
  TonCenter, and additional fork-mode coverage was added for scripts and
  test-runner flows.
- Mutation testing now checks that the baseline test run is green before
  mutating and gives clearer output when filtering selects no baseline tests.
- JUnit and TeamCity reports now include richer captured stdout/stderr,
  location hints, skip/TODO details, and failure context.
- The dot reporter prints clearer runtime and failure details, avoids gas
  snapshot noise when tests fail, and benefits from faster message processing.
- Test runner filesystem helpers and snapshot APIs now reject absolute paths,
  parent-directory escapes, and symlink escapes outside the project root.
- Coverage excludes `.test.tolk` files by default, handles very large VM logs
  better, and keeps the branch-coverage work from 0.3.0 available in the normal
  reporting flow.
- Stdlib gained `SendResultList.waitForTrace()`, interactive `promptInt` and
  `promptAddress`, better non-interactive prompt fallbacks, array `.map()`,
  `.filter()`, and `.each()`, `BASECHAIN`, state-init search parameters, and
  external-in transaction body/message decoding.
- `expect().toEqual()` and `expect().not.toEqual()` now compare typed
  values instead of raw tuple layouts, which fixes nullable struct and union
  equality and produces clearer diffs for nested structs, arrays, and top-level
  union cases.
- Fixed `Expectation<map<K, V>>.toHaveLength` value ordering and improved
  `net.isDeployed`, matcher behavior, bounce opcode handling, empty-data
  opcode loading, and typed mismatch rendering for `env.slice()` values.

### Debugging, Tracing, and UI

- Console transaction trees, retrace, Test UI, and localnet explorer views now
  cover external-in, tick-tock, reserve, send-message, `setCode`, and
  `changeLibrary` actions with richer ABI-decoded bodies, opcode chips, mode
  descriptions, source locations, failure context, and fallback rendering.
- On-demand disassembly is available for `setCode` and embedded
  `changeLibrary` actions, and `acton disasm --json` can emit machine-readable
  disassembly with source-map ranges.
- `acton disasm` is more tolerant of malformed or partial code slices: invalid
  opcodes and undecompilable inline/ref code are emitted as `embed x{...}`
  slices, dictionary decompilation falls back to raw cells when needed, slice
  output uses stable uppercase hex, and real-world TASM reference fixtures were
  added for regression coverage.
- Localnet v3 trace lookup now supports `msg_hash`-based discovery and
  `sendBocReturnHash` normalization, which also powers
  `SendResultList.waitForTrace()`.
- Storage diffs, parsed maps, state-init views, tree tooltips, trace selectors,
  and large trace handling were made more readable and less layout-sensitive.
- Transaction tree formatting now handles contracts created with `fromAddress`
  more clearly, and script debugging works with those contracts.
- Debugger stepping and rendering were improved for child VMs, stop requests,
  parent-frame locals, invalid-message stops, union type display, and
  Compiler-ABI-based decoding.
- Retrace output now handles transactions with skipped compute phases and
  transactions without message bodies more reliably.
- The UI stack was moved to the Compiler ABI model, gained clearer action code
  readability, fixed optional coverage loading, restored missing theme
  behavior, and addressed security audit findings.

### Tolk, Formatting, Linting, and Build

- Acton now relies on compiler ABI metadata for contract ABI, `println`, and
  `format` handling instead of the deprecated tree-sitter-based `ContractAbi`
  path.
- Tree-sitter, resolver, formatter, and wrapper generation now understand
  annotated struct fields and typed `@abi.clientType(...)` declarations.
- `tolk-fmt` now preserves user-authored line breaks in function calls,
  function parameter lists, and union type aliases, and handles file header
  comments, single-string annotations, struct field annotations, simple literal
  calls, and type instantiation formatting more predictably.
- `acton fmt` now supports `--range startLine:startChar-endLine:endChar` for
  editor integrations that need to format only a selected UTF-8 byte range in a
  single `.tolk` file, and range formatting keeps surrounding nodes and import
  order untouched.
- `tolk-fmt` no longer rewrites explicit struct literal fields like `foo: foo`
  into shorthand `foo`; the linter remains responsible for suggesting that
  style change when appropriate.
- Added and refined `acton check` inspections for explicit `.toCell()` inside
  `createMessage({ body: ... })`, documented enum values used in `throw ...`
  paths, dict-type usage, unsafe send/reserve patterns, and related
  style/error classifications.
- Send-mode and reserve-mode literal autofixes now emit bitwise `|`
  expressions, and existing numeric `|` expressions are normalized to named
  mode constants when all bits are recognized.
- `acton meta get-schema` now exposes schemas for custom mutation rules and
  linter JSON reports.
- Linter JSON, GitLab, and SARIF output include richer fix applicability and
  rule metadata, and the documentation generator now records source paths for
  generated linter rule pages.
- Build output now separates compiler ABI artifacts into `build/abi/`, supports
  `[build].output-abi` and `--output-abi`, and can skip automatic `.acton/`
  stdlib installation with `ACTON_DISABLE_AUTO_STDLIB`.

### JetBrains Plugin

The separate TON plugin for JetBrains IDEs also moved during the
`0.3.0 -> 0.4.0` window.

- Acton setup in the IDE is more self-contained: the plugin can discover Acton
  from the default `~/.acton` install location, warns when an Acton project has
  no usable executable, offers installer/configuration/docs actions, and can
  set up a missing project stdlib through `acton init --stdlib-only` or the
  first `acton build`.
- Acton actions now work better in monorepos. File-based features resolve the
  nearest `Acton.toml`, Tolk stdlib detection is context-aware for nested Acton
  projects, contract/script/run/test/retrace completions use that context, and
  Windows paths and test-location parsing were tightened.
- Contract gutters, `Acton.toml` gutters, and `Acton.toml` context actions
  gained direct paths for building contracts, disassembling contract code,
  regenerating all Tolk or TypeScript wrappers, and initializing a dApp with
  `acton init --create-dapp`.
- The assembly preview was rebuilt around `acton compile --source-map` and
  `acton disasm --json`, with a dedicated read-only assembly editor, source to
  assembly block mapping, refresh states, and clearer failure rendering.
- `acton fmt` integration now supports fragment/range formatting using the
  same zero-based UTF-8 byte range format as the CLI.
- Debug and test ergonomics improved with declaration-hover value evaluation,
  rerun-selected-test support in the test tree, Tolk file path console links,
  and cleaner parameter hints for noisy helpers such as `format`, `send`,
  `expect`, `println`, `address`, and `ton`.
- Tolk language support now understands annotated struct fields, dotted
  `@abi.*` annotations, type arguments inside `@abi.clientType(...)`, the
  newer contract header fields, alias-field completion, enum value inlay hints,
  shorter import-mapping paths, and improved TLB reference resolving.

### VS Code Extension

The official TON extension for VS Code also moved during the same
`0.3.0 -> 0.4.0` window.

- Acton setup became more automatic: the extension detects project
  `Acton.toml` files, resolves Acton from the default `~/.acton/bin/acton`
  location before falling back to `PATH`, prompts for install/configuration/docs
  when Acton is missing, and records the configured path after a successful
  install.
- Tolk contract code lenses now expose build, Tolk wrapper generation, and
  TypeScript wrapper generation actions, while `Acton.toml` wrapper sections
  gained code lenses for regenerating all configured Tolk or TypeScript
  wrappers.
- VS Code formatting now passes selected ranges through to `acton fmt --range`
  with zero-based UTF-8 byte columns instead of refusing range formatting.
- BoC and sandbox disassembly now goes through `acton disasm` instead of the
  bundled `ton-assembly` package, aligning VS Code output with the CLI and the
  new disassembler behavior.
- Acton quick-fixes now save the files they edit and rerun checks for the active
  document, while the language server handles external file creates, updates,
  deletes, stale duplicate events, and encoded `@` paths more reliably.
- Tolk language support caught up with the latest surface: annotated struct
  fields, dotted annotations, type-valued annotation arguments, the
  `@abi.clientType(...)` shape, removal of `symbolsNamespace` contract-header
  completion, a quick action for generating 32-bit struct opcodes, less noisy
  parameter hints, and more robust completion in incomplete expressions and
  import-mapping-heavy projects.

### Documentation, CI, and Internal Polish

- Documentation gained a refreshed landing page, video previews, linter error
  previews, how-to guides for formatting and linting, updated CI setup docs,
  all testing articles, 404 handling, `robots.txt`, `llms.txt` pages, and
  updated install URLs.
- The docs site now generates framework-native `robots` and `sitemap` routes,
  includes `sitemap.xml` in robots output, requires page descriptions, and
  filters hidden pages out of the sitemap.
- Docs validation now checks navigation, external links, typos, formatting, and
  generated command/rule references more aggressively.
- The docs site received refreshed styling, footer and navigation updates,
  theme fixes, OG image updates, PR preview support, and fewer hidden or stale
  pages.
- Project logging now rotates logs, suppresses unnecessary debug-log warnings
  when the default log path is unavailable, and reports relevant proxy and
  stdlib environment variables through `acton doctor`.
- Internal crate names were normalized, native objects and dependencies were
  refreshed, stricter clippy rules were enabled, and CI/cache behavior was
  tightened around docs, generated artifacts, checksums, security updates, and
  template consistency.
- The JetBrains and VS Code editor repositories added or tightened Zizmor-based
  GitHub Actions security checks during the same release window.

## [0.3.2] - 27.04.2026

Acton 0.3.2 expands project scaffolding, wrapper generation, disassembly,
script argument handling, and stdlib test APIs while tightening formatter,
linter, debugger, localnet, and reporting behavior. It also refreshes the
documentation site and fixes several wrapper, template, logging, and UI issues
found after 0.3.1.

### Added

- Added `acton init --create-dapp` for scaffolding Vite-based TypeScript apps,
  along with app templates and generated TypeScript wrappers for Counter,
  Jetton, and NFT projects.
- Added `acton meta get-schema` support for the custom mutation rules schema
  and the lint JSON report schema.
- Added `SendResultList.waitForTrace()` and localnet v3 trace lookup support,
  including `msg_hash`-based trace discovery.
- Added stdlib support for interactive `promptInt` and `promptAddress`, array
  `.map()`, `.filter()`, and `.each()`, `BASECHAIN`, external-in transaction
  body/message decoding, and state-init search parameters.
- Added a linter inspection for documenting enum values used in `throw ...`
  paths. The inspection is available but allowed by default.
- Added `acton disasm --json`, which returns machine-readable disassembly with
  optional source-to-assembly mapping ranges from `--source-map`.
- Added ABI-driven parsing and validation for `acton script` arguments, with
  clearer support and error reporting for arrays, nullable values, addresses,
  strings, cells, and other CLI-passed types.
- Added tree-sitter support for annotated struct fields and typed
  `@abi.clientType(...)` declarations.

### Changed

- Generated wrappers now use `.gen` filenames, avoid unused imports, include
  explicit return types, and stop suppressing linter and formatter checks by
  default.
- Counter, Jetton, and NFT templates were refined with unified contract
  sources, `.env.example`-based setup, clearer wrapper regeneration flows,
  prettier README/tests/scripts, app flows, owner checks, and consistency
  checks for generated wrappers.
- The dot reporter now prints richer failure/runtime details, supports more
  cases with colors, and benefits from faster message processing.
- Coverage and VM log handling now scale better for very large VM logs.
- `tolk-fmt` now handles file header comments, single string annotations,
  simple literal function calls, and type instantiation formatting more
  predictably.
- Debugger stepping and rendering were improved for child VMs, stop requests,
  parent-frame locals, invalid-message stops, and union type display.
- Documentation gained a refreshed landing page, video previews, linter error
  previews, link validation, PR previews, and updated install URLs.
- Internal crate names were normalized to the current kebab-case naming style.
- `state_init` now uses `Cell<StateInit?>` to match the latest Tolk 1.4
  expectations.

### Fixed

- Fixed wrapper generation around unused imports and shard address calculation.
- Fixed NFT and Jetton template issues, including TonCenter key handling in app
  templates.
- Fixed the Counter app template by aligning wallet flows with TonConnect UI.
- Fixed `net.isDeployed` and related matcher logic for prefunded and
  deterministic deploy paths.
- Fixed empty-data handling in `TlbMessageRelaxedGeneric.loadOpcode` and
  improved bounced opcode matching for the new prefix format.
- Fixed logging setup and addressed a UI security audit finding.
- Fixed docs cache and missing documentation pages in CI.

## [0.3.1] - 23.04.2026

Acton 0.3.1 is a focused follow-up to 0.3.0. It improves the test runner and
both UI surfaces, expands transaction and action inspection in the Test UI and
localnet explorer, and smooths a handful of scripting, formatting, and docs
rough edges.

### Added

- Added support for `@test.skip("description")` in the test runner. Skip and
  TODO reasons now flow through console output, the Test UI, JUnit, and
  TeamCity reporting.
- Added Nushell support to `acton completions`.
- Added a new lint inspection that warns about explicit `.toCell()` inside
  `createMessage({ body: ... })`, where the extra conversion is usually
  unnecessary.
- Added on-demand disassembly for `setCode` and embedded `changeLibrary`
  actions in the Test UI and localnet explorer.

### Changed

- Expanded Test UI and localnet explorer transaction views for
  `external-in` and `tick-tock` flows, including better root visualization,
  richer phase and action details, and better handling of large traces.
- Send-message actions now show ABI-decoded bodies, opcode chips, clearer
  send-mode descriptions, and better fallback handling for raw and bounced
  payloads.
- `reserve`, `setCode`, and `changeLibrary` actions now render with clearer
  mode details, failure context, and code/library inspection.
- Parsed maps and storage diffs are now rendered more readably in transaction
  details and tree tooltips, and oversized trace selectors/tooltips are now
  scrollable instead of stretching the layout.
- `tolk-fmt` now preserves user-authored line breaks in function calls,
  function parameter lists, and union type aliases.
- `acton up` no longer special-cases Homebrew-style installation paths.

### Fixed

- Fixed `net.sendExternal()` on real networks and cleaned up the surrounding
  wait-for-transaction flows used by templates and scripts.
- Fixed a `404` on optional coverage loading in the Test UI.
- Fixed several broken docs links and generated documentation references.

## [0.3.0] - 20.04.2026

Acton 0.3.0 focuses on cleaning up and consolidating the surface introduced in
0.2.0. It renames several CLI and manifest concepts around broadcasting,
local execution, wrappers, and imports; reorganizes the Acton stdlib; and adds
a richer localnet explorer, much stronger debugger output, better coverage and
test-runner performance, Tolk 1.4 support, and a new NFT starter template.

### Breaking Changes and Migration

- `acton script` no longer uses `--broadcast`. Passing `--net <network>` now
  both selects the live network and enables real broadcasting. Local emulation
  is still the default when `--net` is omitted.

  ```bash
  # before
  acton script scripts/deploy.tolk --broadcast --net testnet
  
  # after
  acton script scripts/deploy.tolk --net testnet
  ```

  If you previously used `--broadcast` in project scripts, README snippets, CI
  jobs, or shell aliases, remove it everywhere. If you still want local
  execution against remote state, keep using `--fork-net` without `--net`.

- TonCenter authentication is now environment-only and split by network.
  User-facing `--api-key` flags and the `[test].api-key` config field were
  removed. Use `TONCENTER_TESTNET_API_KEY` for testnet flows and
  `TONCENTER_MAINNET_API_KEY` for mainnet flows.

  ```bash
  # before
  acton test --fork-net testnet --api-key YOUR_API_KEY
  acton script scripts/deploy.tolk --net mainnet --api-key YOUR_API_KEY
  
  # after
  TONCENTER_TESTNET_API_KEY=YOUR_API_KEY acton test --fork-net testnet
  TONCENTER_MAINNET_API_KEY=YOUR_API_KEY acton script scripts/deploy.tolk --net mainnet
  ```

  Built-in `mainnet`/`testnet` commands now pick the matching env var
  automatically. For `custom:<name>`, Acton reads
  `<NORMALIZED_NAME>_API_KEY` instead, for example `custom:foo-bar` ->
  `FOO_BAR_API_KEY`. The old shared `TONCENTER_API_KEY` fallback is gone.

- `acton litenode` was renamed to `acton localnet`, and the manifest section
  `[litenode]` was renamed to `[localnet]`. The network name stays `localnet`,
  so `--net localnet` and `[networks.localnet]` do not change.

  ```toml
  # before
  [litenode]
  port = 3010
  fork-net = "testnet"
  
  # after
  [localnet]
  port = 3010
  fork-net = "testnet"
  ```

  Update CLI commands, docs links, helper scripts, and config lookups that
  still refer to `litenode`.

- `Acton.toml` renamed `[mappings]` to `[import-mappings]`.

  ```toml
  # before
  [mappings]
  wrappers = "tests/wrappers"

  # after
  [import-mappings]
  wrappers = "wrappers"
  ```

  The old section name is not the canonical config surface anymore, so rename
  it instead of keeping compatibility shims in downstream tooling.

- Contract config field `name` was renamed to `display-name`. The
  `[contracts.<NAME>]` key is now the canonical contract name used for CLI
  selection, dependency naming, helper generation, and wrapper generation;
  `display-name` is an optional UI/log label only.

  ```toml
  # before
  [contracts.counter]
  name = "Counter"
  src = "contracts/counter.tolk"
  
  # after
  [contracts.Counter]
  display-name = "Counter"
  src = "contracts/Counter.tolk"
  ```

  New scaffolds now use PascalCase contract names and filenames for consistency,
  but the hard migration requirement for existing projects is the
  `name -> display-name` rename. If you keep older contract keys, helper file
  names and generated function names continue to follow those keys.

- Default generated Tolk wrapper locations moved from `tests/wrappers/` to
  `wrappers/` in standard layouts, and from `contracts/tests/wrappers/` to
  `contracts/wrappers/` in `--app` layouts. The default `@wrappers` mapping was
  updated accordingly.

  If your tests, scripts, editors, or CI still import or watch the old paths,
  either move the files or pin the legacy layout explicitly:

  ```toml
  [wrappers.tolk]
  output-dir = "tests/wrappers"
  test-output-dir = "tests"
  
  [import-mappings]
  wrappers = "tests/wrappers"
  ```

- Default generated TypeScript wrapper output moved from `wrappers/` to
  `wrappers-ts/`.

  If your frontend imports from the old directory, either update the import
  path or pin the old output directory:

  ```toml
  [wrappers.typescript]
  output-dir = "wrappers"
  ```

- Generated dependency helper files were renamed from
  `<dependency>_code.tolk` to `<dependency>.code.tolk`.

  ```text
  # before
  gen/jetton-wallet_code.tolk
  
  # after
  gen/JettonWallet.code.tolk
  ```

  Update any checked-in generated helpers, import statements, globs, and
  scripts that reference the old `_code` suffix.

- The test runner now recognizes only dotted `@test.*` annotations. Legacy
  object-style `@test({...})` forms are ignored.

  ```tolk
  // before
  @test({ skip: true })
  @test({ todo: "later" })
  @test({ fail_with: 42 })
  @test({ gas_limit: 1000 })
  @test({ fuzz: { runs: 64, seed: 42 } })
  
  // after
  @test.skip
  @test.todo("later")
  @test.fail_with(42)
  @test.gas_limit(1000)
  @test.fuzz({ runs: 64, seed: 42 })
  ```

  Update all existing test sources before relying on skip/todo/fail/fuzz
  behavior in 0.3.0.

- `acton test` and `acton script` no longer print low-level executor debug
  logs by default. If you relied on the old always-verbose behavior for CI,
  snapshots, troubleshooting, or `debug.dumpStack()` output, pass `-v` /
  `--verbose` explicitly.

  ```bash
  # before
  acton test
  acton script scripts/debug.tolk
  
  # after, to keep the old debug-log-heavy output
  acton test -v
  acton script scripts/debug.tolk --verbose
  ```

  Verbosity above one level is not supported yet, so use `-v` once instead of
  `-vv`.

- Lint suppression comments were renamed from
  `// acton-disable-next-line <rule>` to
  `// check-disable-next-line <rule>`.

  ```tolk
  // before
  // acton-disable-next-line unused-variable
  
  // after
  // check-disable-next-line unused-variable
  ```

- The Acton stdlib import surface was reorganized in
  [#849](https://github.com/ton-blockchain/acton/pull/849). Several top-level
  modules were flattened, several testing/emulation APIs moved into
  better-scoped modules, and a few legacy paths were removed.

  | Before                              | After                                                                                                            | Notes                                                                        |
  |-------------------------------------|------------------------------------------------------------------------------------------------------------------|------------------------------------------------------------------------------|
  | `@acton/build/build`                | `@acton/build`                                                                                                   | Flat import path                                                             |
  | `@acton/crypto/crypto`              | `@acton/crypto`                                                                                                  | Flat import path                                                             |
  | `@acton/ffi/ffi`                    | `@acton/ffi`                                                                                                     | Flat import path                                                             |
  | `@acton/promts/prompts`             | `@acton/prompts`                                                                                                 | Fixed typo and flattened path                                                |
  | `@acton/testing/transaction_expect` | `@acton/emulation/network` plus `@acton/testing/expect`                                                          | Transaction matchers now live with `SendResultList`                          |
  | `@acton/testing/outlist_expect`     | `@acton/types/out_actions` plus `@acton/testing/expect`                                                          | Out-action matchers now live with out-action types                           |
  | `@acton/emulation/tracing`          | `acton test --save-test-trace`, `SendResultList.giveName(...)`, and the saved-trace/Test UI workflows            | Trace export moved out of the old stdlib module                              |
  | `@acton/vm/vm`                      | use the specialized replacements in `@acton/emulation/testing`, `@acton/emulation/network`, and `@acton/types/*` | For example, `vm.registerLibrary(...)` became `testing.registerLibrary(...)` |

  If you maintain custom helper libraries or examples, search for the removed
  paths directly and rewrite imports rather than relying on transitive aliases.

- Build-owned generated artifacts now live under `build/` instead of `.acton/`.
  This affects shared compilation cache, saved traces, mutation sessions, and
  project-local logs:

  | Before                     | After                     |
  |----------------------------|---------------------------|
  | `.acton/cache`             | `build/cache`             |
  | `.acton/traces`            | `build/traces`            |
  | `.acton/mutation-sessions` | `build/mutation-sessions` |
  | `.acton/logs`              | `build/logs`              |

  `.acton/` still remains the home of the bundled Acton stdlib and other
  project-managed support files, so do not delete it. Update CI caches,
  cleanup scripts, editor integrations, and `.gitignore` rules that still
  assume build-owned artifacts live under `.acton/`.

- Raw Tolk compiler JSON now follows upstream `snake_case` field names instead
  of `camelCase`. This is primarily a breaking change for custom integrations
  that consume compiler ABI or source-map JSON directly. If you only use
  `acton build`, `acton wrapper`, or the bundled TypeScript generator, migrate
  your own tooling only where it reads the raw compiler payloads.

### Localnet, CLI, and Project Workflows

- Replaced the old `litenode` surface with `localnet` across the CLI, docs,
  config schema, manpages, and internal crates, making the terminology match
  the already-existing `localnet` network name.
- Added a bundled localnet explorer UI with better account pages, ABI-aware
  contract display, wallet support in `v3/accountStates`, account type
  reporting, opcode display, and broader TonCenter v3 compatibility.
- Added state persistence controls for localnet startup and shutdown via JSON
  load/dump flows, plus clearer localnet wallet-airdrop guidance across CLI
  errors.
- Script and run flows now default explorer links to `tonscan`; pin
  `--explorer` explicitly if your docs, tooling, or operator workflow relied on
  a different explorer provider.
- Added `acton up --force` to reinstall the currently selected version even
  when it already matches the installed build.
- Added a new NFT template with collection, item, wrappers, tests, and
  deployment scripts, and simplified `acton new` with optional advanced setup
  prompts and a hidden `--templates` catalog mode for IDEs and tooling.
- New scaffolds now default to PascalCase contract names and filenames, and the
  generated project scripts and docs were updated to the new wrapper and
  broadcast conventions.
- Shell completion, path completion, and command reference generation were
  improved across `script`, `wrapper`, and top-level command help.
- Improved `acton script` exit-code failure output with clearer failure phases,
  descriptions, and actionable follow-up hints around backtraces and wallet or
  deployment setup issues.

### Testing, Coverage, and Stdlib

- Refactored the Acton stdlib into a clearer module layout, splitting
  emulation-heavy APIs across `@acton/emulation/network`,
  `@acton/emulation/testing`, and `@acton/emulation/scripts`, and moving
  matcher APIs closer to the types they operate on.
- Added branch coverage to the console coverage table and LCOV output, and
  significantly reduced coverage memory use on larger test suites.
- Build caching now excludes Fift output from default cache entries, reducing
  cache size and warm-load overhead while still generating Fift when explicitly
  requested.
- `acton test` now hides executor debug logs by default and exposes `-v` for
  low-level executor output when needed, reducing noisy output and memory use in
  normal runs.
- Added support for loading libraries from the emulated blockchain via
  `net.loadLibrary(...)`, and `net.getConfig()` now returns the real blockchain
  config in broadcast mode.
- Improved matcher ergonomics with support for function-valued matchers and
  faster search in larger send-result lists.
- Scripts can now use matcher helpers directly, and matcher evaluation now
  works correctly with FFI-backed helpers.
- Added universal `println` and `format` helpers across the scripting and
  testing surface, wallet helper APIs for exposing wallet key pairs and wallet
  IDs from open broadcast wallets, prompt-library improvements for
  non-interactive environments, and stronger `parse*` implementations in
  `fmt`.

### Debugging, Compiler, and Language Tooling

- Expanded the debugger with evaluate requests, conditional breakpoints,
  better JetBrains and VS Code behavior, raw-address display, TON-aware coin
  rendering, richer exception naming, and substantially improved rendering for
  typed storage, `Cell<T>`, maps, unions, enums, strings, slices, builders,
  out actions, and inbound messages.
- Debugging flows now better support custom network config, missing libraries,
  external inbound messages, and ABI-based decoding during replay.
- Added support for Tolk 1.4 closures, improved lambda formatting, fixed tuple
  and tensor parsing edge cases, added support for numeric separators like
  `1_000_000`, and aligned compiler JSON with upstream Tolk output.
- Debugger previews now do a better job with non-loaded lazy fields by
  deserializing slices when possible.
- Added `acton compile --allow-no-entrypoint` for compiler/debugging workflows
  that intentionally compile files without a contract entrypoint.
- Added new `acton check` inspections for dict-type usage and for preferring
  `throw Errors.ErrorName` over raw `throw CONST_NAME`, plus richer exported
  rule tags including `Deprecated`.
- Runtime error reporting now uses compiler ABI metadata to show source-level
  error names more consistently across tests, scripts, and debugger output.

### JetBrains Plugin

The separate TON plugin for JetBrains IDEs also moved materially during the
same `0.2.0 -> 0.3.0` window.

#### Compatibility and Scope

- Compatibility changed in a user-visible way: the plugin dropped Blueprint
  support and now targets JetBrains `2025.2+` IDE builds, with RustRover used
  as the base development platform.

- The Acton project wizard in the plugin now tracks the CLI more closely,
  including broader template/options coverage and loading templates from Acton
  itself to reduce drift between IDE-created and CLI-created projects.

#### Acton.toml and Editor Support

- The plugin was updated to understand the newer Acton surface: the latest
  `Acton.toml` schema, profiling-related config, the newer script broadcasting
  model based on `--net`, the updated `tonscan` default, dotted `@test.*`
  annotations, and newer Acton stdlib helper functions.

- `Acton.toml` editing became much stronger: schema coverage was refreshed,
  completion and reference resolution improved, script entries gained language
  injection, and the IDE now provides more useful gutters for `[fmt]`,
  `[lint]`, `[test]`, and contract build actions directly from source files or
  manifest entries.

- Tolk language support in the plugin also moved forward with the Tolk 1.4
  wave: dotted annotations, `void` type parameters, early lambda completion,
  highlighting for captured lambda variables, and parser fixes such as tensor
  types with trailing commas.

#### Run, Debug, and Test UX

- IDE run/debug flows expanded substantially. The plugin now supports DAP-based
  debugging for `acton script`, `acton test`, and `acton retrace`.

- Debug ergonomics improved too: the IDE can show debug values on variable or
  field hover, offers one-click rerun with `--backtrace full`, and surfaces
  clearer failed-test inspections with actual vs expected values.

- Test feedback became more robust: the plugin adds rerun-failed-tests support
  and keeps failed-test and failed-`expect` underlines stable after source
  edits instead of dropping the failure context too aggressively.

- Acton command execution inside the IDE now uses terminal-like console / PTY
  integration, which makes prompt-driven commands and interactive flows work
  much better from run configurations.

#### Coverage and Profiling

- Coverage became more IDE-native: the plugin now imports LCOV branch-hit data
  into the JetBrains coverage model and improves coverage report generation.

- Initial CPU profiling support also landed for `acton test` in IDEs where the
  JetBrains profiler APIs are available.

### VS Code Extension

The official TON extension for VS Code also moved noticeably during the same
`0.2.0 -> 0.3.0` window.

#### Acton and Project Awareness

- The extension was updated to understand the newer Acton surface: the latest
  `Acton.toml` changes, the switch to `--net`-based broadcasting, the `tonscan`
  default, and newer Acton stdlib helper names such as `scripts.wallet()`.

- It also now derives the displayed Tolk version from Acton itself when working
  inside an Acton project, which reduces confusion when the workspace toolchain
  differs from a separately installed global Tolk.

#### Run, Debug, and Retrace

- VS Code gained proper Acton debugging support for tests and scripts, instead
  of only basic run flows.

- A new retrace workflow was also added: the extension can now start source
  debugging for a real on-chain transaction by asking for the hash, network,
  and contract from `Acton.toml`, then launching `acton retrace --debug`.

- `Acton.toml` code lenses became more capable too, with direct actions for
  `[fmt]`, `[check]`, and `[test]`, including a dedicated test-UI run path from
  the manifest.

#### Test and Diagnostic UX

- Test failure reporting in the VS Code test explorer became much more useful:
  failures now preserve source locations better and can surface structured
  `expected` / `actual` output when Acton provides it through TeamCity-style
  test events.

- Acton linter integration was substantially hardened. The extension now does a
  better job canceling stale checks, avoiding diagnostics for dirty buffers,
  mapping related annotations, surfacing tags such as `Deprecated` /
  `Unnecessary`, and exposing Acton quick-fixes more reliably as VS Code code
  actions.

- The extension also removed one overlapping built-in call-argument inspection
  so that `acton check` is the primary source of truth for those diagnostics,
  which should reduce duplicate or contradictory warnings.

#### Tolk Language Support

- Tolk support in the language server kept pace with the same language wave:
  `void` type parameters from Tolk 1.4 are understood, completion hides
  internal `__*` symbols, and contracts stop surfacing `.acton` symbols or
  `.acton` import suggestions where they are just noise.

### Docs, Distribution, and Release Tooling

- Switched project and docs UI tooling from `yarn` toward `bun`, added Bun
  caching in CI, and added a dedicated VS Code extension build workflow.
- Added automatic documentation deployment and broader release-hardening checks,
  including installer validation and release security checks.
- Expanded documentation around debugging, build caching, localnet, wrapper
  generation, saved trace bundles, and the reorganized stdlib surface.

### Upgrade Checklist

- Rename `--broadcast` usages to `--net`.
- Rename `[litenode]` to `[localnet]`.
- Rename `[mappings]` to `[import-mappings]`.
- Rename contract `name` to `display-name`.
- Rewrite legacy `@test({...})` annotations to dotted `@test.*` forms.
- Add `-v` / `--verbose` anywhere your tests or scripts relied on raw executor
  logs or `debug.dumpStack()` output by default.
- Rewrite `acton-disable-next-line` comments to `check-disable-next-line`.
- Update wrapper paths, generated helper file names, and any `@wrappers`
  mappings or globs that still point at `tests/wrappers` or `_code.tolk`.
- Update stdlib imports for the flattened/reorganized 0.3.0 module layout.
- Update CI caches and cleanup scripts to use `build/cache`, `build/traces`,
  `build/mutation-sessions`, and `build/logs`.
- If you consume raw compiler JSON, update your field accessors from
  `camelCase` to `snake_case`.

## [0.2.0] - 06.04.2026

Acton 0.2.0 rolls up all user-facing work shipped after 0.1.0 into a much more
complete beta release. It expands installation and distribution options, adds
built-in manuals and remote inspection, makes wallet and network workflows
safer, upgrades the test runner with snapshots, source-level debugging,
coverage, fuzzing, and mutation testing, and substantially hardens
verification and release tooling.

### Distribution and Installation

- Bundled TON objs files in releases, added an official Docker image workflow,
  published Docker installation docs, and included a simple GitLab CI example
  for containerized usage.
- Added contributor helpers for native artifacts via `cargo xtask objs` and
  `just sync-artifacts`, simplifying local TON objs bootstrap and refresh
  workflows.
- TON objs archive validation now uses the checked-in
  `artifacts_manifest.toml`, with `TON_OBJS_DISABLE_ARCHIVE_SHA_VERIFY=1`
  available as an escape hatch for local archive refresh workflows.
- Linux TON objs builds no longer use `-march=native`, improving compatibility
  on older CPUs, and the checked-in TON objs plus artifact manifests were
  refreshed.
- Release and distribution workflows were hardened around published artifacts,
  attached TON objs files, binary compatibility checks, cross-architecture
  validation, and QEMU-based artifact verification.

### Docs, Templates, and CLI UX

- Added long-form built-in manuals via `acton help <command>`, plus bundled
  plain-text manual artifacts and generated manpages.
- Expanded and corrected user documentation across Docker, debugging,
  quickstart, wallet examples, and the test runner, including a dedicated
  step-by-step execution guide.
- Added `acton new --agents` with template-specific `AGENTS.md` files, a
  direct `Acton.toml` documentation link in generated templates, and updated
  the `jetton` starter template for the latest Tolk 1.3 syntax with a
  corrected generated `deploy.tolk` script.
- Starter `deploy.tolk` scripts now read back deployed state after deploy or
  mint flows, so generated projects verify post-deploy state immediately.
- Added richer CLI and script diagnostics, including better busy-port errors,
  explicit descriptions for exit code `0xFFFF`, script failure phases, and
  source backtraces when re-run with `--backtrace full`.
- Added `acton doctor` checks for common backend API availability, native `.a`
  library versions, embedded TON commit metadata, and cache-directory
  reporting; reachability checks now also degrade gracefully in restricted
  sandboxed environments instead of failing with opaque proxy-discovery
  panics.
- `acton up` now detects Unicode dashes pasted into flags, reducing copy-paste
  failures from Telegram and similar sources.
- Build, compile, and test commands now treat artifact write failures as hard
  errors instead of warnings.
- `acton check` now lints standalone script roots that define `main()`.
- Fixed `acton check --output-format json` to report the `success` field
  correctly.

### Wallets, RPC, and Network Workflows

- Added `acton rpc info` for remote account inspection, status and hash
  reporting, `code_hash` matching against local contracts, and decoded on-chain
  storage when local ABI metadata is available.
- Secure wallet storage now keeps per-scope mnemonic bundles in the native
  keychain, allowing multiple secure wallets in one scope to share a single
  keychain entry and be updated or removed independently.
- Interactive testnet airdrops in `acton wallet new` and
  `acton wallet airdrop` now wait briefly for balance confirmation by default,
  with `--no-wait-airdrop` available to skip the wait, and wallet creation and
  import output now includes clearer balance-check follow-ups.
- Broadcast and real-network send flows now poll more aggressively after
  submission, surface clearer failure diagnostics for missing wallet state,
  insufficient balance, stale `seqno`, expired messages, and Toncenter
  `send_boc` failures, and reject `net.treasury` in broadcast mode.
- `acton script --net <network>` now defaults remote state reads to the selected
  broadcast network when `--fork-net` is omitted, and rejects conflicting
  `--net` / `--fork-net` combinations.

### Testing, Coverage, and Mutation

- Added test-runner APIs `net.sendIter()` and `TxCursor` for stepwise
  execution, plus `net.saveSnapshot()` and `net.loadSnapshot()` for JSON
  world-state snapshots.
- Added opt-in fuzz testing for parameterized `.test.tolk` tests via
  `@test({ fuzz: ... })`, project defaults in `[test.fuzz]`,
  `acton test --fuzz-seed`, and `fuzz.assume(...)` / `fuzz.bound(...)`
  helpers.
- Added stronger coverage controls and reporting: a Test UI coverage view,
  project-level `[test.coverage]` settings, `--coverage-include-tests`,
  `--coverage-include-wrappers`, and `--coverage-minimum-percent` for CI
  gating.
- Added mutation-rule filtering, severity gating, and extensibility via
  `--mutation-levels`, `--mutation-minimum-percent`, custom JSON rules with
  `--mutation-rules-file`, additional built-in rules, changed-line scoping via
  `--mutation-diff` / `--mutation-diff-ref`, resumable sessions with
  `--mutation-session-id`, and targeted reruns with `--mutation-id`.
- Coverage collection is now much more precise, with better branch accounting,
  zero-hit files retained in reports, synthetic end-of-function lines excluded
  from executable counts, and wrappers excluded by default unless explicitly
  requested.
- Build caching now avoids long locks and eager data loading, improving
  repeated build and test workflows.
- Mutation testing now runs in parallel by default with isolated worker
  workspaces, substantially reducing runtime on larger suites;
  `--mutation-workers` can still cap concurrency.
- The mutation-rule disable flag was renamed from `--disable-rule` to
  `--mutation-disable-rules` for consistency with the rest of the mutation CLI
  surface.
- Fixed test-runner `isContractDeployed()` detection for missing and
  explicit-null account states.

### Debugging, Compiler, and Language Tooling

- Added a first-class source-level debugger built on a new debug engine and DAP
  server, with richer value rendering for strings, cells, slices, builders,
  maps, and addresses, runtime exception reporting, and JetBrains
  compatibility fixes.
- Added retrace-driven debugging and improved disassembly on top of new
  compiler source maps, refreshed Tolk 1.3 support, annotation names with
  dots, and Tolk file formatting support across the toolchain.
- Added standard-library improvements including `println2` through `println5`,
  `env()` support for `coins`, and automatic stdlib refreshes on trunk
  updates.

### Verification, Reliability, and Security

- Verification flows now retry transient backend failures, honor
  signer-backend overrides during signature collection, surface backend
  response bodies and parse errors, produce clearer dry-run and send output,
  print clearer success output when the verification proof is already
  deployed, and link more consistently to mainnet and testnet verifiers.
- Fixed verification edge cases around unsupported networks, backend error
  handling, and transaction-send failures, reducing opaque failures during
  real-network verification workflows.

### Upgrade Notes

- If you use secure wallets backed by the native keychain, re-import or
  recreate them after upgrading so Acton can rewrite the stored mnemonic data
  in the bundled format.
- If you store coverage settings in `Acton.toml`, move them under
  `[test.coverage]`.
- If you use mutation scripts or CI jobs, rename `--disable-rule` to
  `--mutation-disable-rules`.
- If you want reproducible fuzz runs in CI, set `[test.fuzz].seed` in
  `Acton.toml` or pass `acton test --fuzz-seed <SEED>` explicitly.
- If your CI uses the GitHub Action, update workflow references to
  `ton-blockchain/setup-acton`.

## [0.1.4] - 29.03.2026

Acton 0.1.4 adds remote account inspection and deeper test-runner control,
while improving wallet, broadcast, and diagnostics workflows around real-network
interactions.

### Added

- Added `acton rpc info` to inspect a remote account, print status and hash metadata, match local contracts by `code_hash`, and decode on-chain storage through local ABI metadata when available.
- Added iterative test-runner execution via `net.sendIter()` and `TxCursor` for partial transaction-chain execution, targeted stopping, and interleaving multiple message cascades against shared emulated state.
- Added world-state snapshot APIs `net.saveSnapshot()` and `net.loadSnapshot()` to persist local emulator state as JSON fixtures and restore it in later test runs.

### Changed

- Changed interactive testnet airdrops in `acton wallet new` and `acton wallet airdrop` to wait briefly for balance confirmation by default, with `--no-wait-airdrop` available to skip the wait.
- Changed broadcast-mode transaction waiting defaults to poll more frequently after submission, reducing unnecessary delay once the message is already on the network.
- Improved real-network send diagnostics across wallet-driven workflows to explain common failures such as missing wallet state, insufficient balance, stale seqno, expired messages, and the right `acton wallet airdrop` fix for testnet or localnet.
- Improved wallet creation and import output with a clearer follow-up hint for checking balances via `acton wallet list --balance`.

### Fixed

- Fixed `acton doctor` API reachability checks to degrade more gracefully in restricted sandboxed environments instead of failing with opaque proxy-discovery panics.
- Fixed test-runner `isContractDeployed()` detection for missing and explicit-null account states.

## [0.1.3] - 28.03.2026

Acton 0.1.3 expands built-in command manuals and diagnostics, improves Linux
binary compatibility on older CPUs, refreshes TON objs metadata, and hardens
CLI workflows with broader integration coverage.

### Added

- Added long-form built-in manuals for top-level commands via `acton help <command>`.
- Added bundled plain-text manual artifacts generated from the CLI definitions.
- Added bundled manpage artifacts generated from the CLI definitions.
- Added API reachability checks to `acton doctor` for common Acton backends.
- Added native `.a` library version reporting to `acton doctor`.
- Added embedded TON commit hash and date reporting for native libraries in `acton doctor`.

### Changed

- Changed Linux TON objs builds to avoid `-march=native`, improving compatibility on older CPUs.
- Refreshed the checked-in TON objs artifact manifest to the latest upstream snapshot.

### Fixed

- Fixed incorrect flag references in the quickstart and test-runner documentation.

## [0.1.2] - 27.03.2026

Acton 0.1.2 improves project scaffolding, script diagnostics, and secure wallet storage,
while tightening release maintenance workflows and TON objs artifact validation.

### Added

- Added `acton new --agents` to include template-specific `AGENTS.md` guidance for coding agents, with matching interactive prompts in project creation flows.
- Added richer `acton script` failure diagnostics, including exit code descriptions and phases, plus source backtraces when re-run with `--backtrace full`.

### Changed

- Changed secure wallet storage to keep per-scope mnemonic bundles in the native keychain, so multiple secure wallets in one local or global scope can share a single keychain entry and be updated or removed independently.
- Changed TON objs archive validation to use the checked-in `artifacts_manifest.toml`, with `TON_OBJS_DISABLE_ARCHIVE_SHA_VERIFY=1` available as an escape hatch for local archive refresh workflows.

### Fixed

- Fixed the `jetton` starter template for the latest Tolk 1.3 syntax and corrected its generated `deploy.tolk` script.
- Fixed a dependency security issue by bumping `tar` to `0.4.45`.

### Upgrade Notes

- If you use secure wallets backed by the native keychain, re-import or recreate them after upgrading so Acton can rewrite the stored mnemonic data in the new bundled format.
- If your CI uses the GitHub Action, update workflow references to `ton-blockchain/setup-acton`.

## [0.1.1] - 22.03.2026

Added TON objs files to releases.

## [0.1.0] - 22.03.2026

Acton 0.1.0 is the first semi-stable beta release with a complete installation and delivery story.
It makes the CLI easy to install from official artifacts while keeping the project on the beta release channel.

### Added

- Added an official shell installer (`acton-installer.sh`) for public beta releases.
- Added official release artifacts for four first-class platforms: macOS (ARM64, x86_64) and Linux GNU (ARM64, x86_64).
- Added a full CI and release pipeline around builds, release validation, artifact checks, public release mirroring, and dependency and security checks.
- Added broader developer tooling over the last few months, including `acton doctor`, `func2tolk`, `Acton.toml` schema generation, better starter templates, and improved TypeScript wrapper generation.

### Changed

- Promoted Acton to the `0.1.x` beta release line.
- Changed the recommended installation flow to use the public installer and official release artifacts.
- Improved `acton up`, templates, wrappers, localnet and network workflows, test reporting, and documentation across the project.

### Fixed

- Fixed numerous issues across CI, release automation, tests, documentation, wrappers, wallets, localnet integration, formatter output, and diagnostics.
- Fixed multiple flaky tests and platform-specific issues, especially around macOS and release workflows.
- Fixed many smaller bugs and polish issues accumulated over the last few months across the CLI, compiler-facing tooling, and project templates.

### Upgrade Notes

- Prefer installing or updating via `acton-installer.sh` or the official release archives.
- First-class public artifacts are available for macOS (ARM64, x86_64) and Linux GNU (ARM64, x86_64).
- If you use generated TypeScript wrappers, note that recent releases now emit them into `wrappers/` by default.

## [0.0.21] - 21.03.2026

### Added

- Added a `counter` starter template with a React + Vite app for `acton new`.
- Added `func2tolk --version`.

### Changed

- Changed generated TypeScript wrappers to go to `wrappers/` by default.

### Upgrade Notes

- If you rely on generated TypeScript wrappers, update any tooling that expected the previous default output location.
- Project references now use the `ton-blockchain/acton` repository path.

### Internal

- Added `cargo xtask schema` to generate the `Acton.toml` JSON schema.
- Added baseline maintainer and project docs, including release, support, security, and conduct policies.
- Improved CI and release automation reliability across release checks and macOS workflows.

## [0.0.20] - 18.03.2026

First version with completed CI.
