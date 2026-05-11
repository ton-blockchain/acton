# Changelog

All notable changes to this project will be documented in this file.

## [Unreleased]

### Added

- No unreleased entries yet.

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
