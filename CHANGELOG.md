# Changelog

All notable changes to this project will be documented in this file.

## [Unreleased]

### Added

- No unreleased entries yet.

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
- `tolkfmt` now preserves user-authored line breaks in function calls,
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
