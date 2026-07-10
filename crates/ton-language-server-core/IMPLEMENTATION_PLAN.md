# ton-language-server-core Implementation Plan

## Goal

Build a new language server architecture that can run from the same core in two
fully local environments:

- native Acton CLI or editor integration;
- browser/Monaco integration through a local Web Worker and WASM.

The legacy language server has been replaced and removed. This implementation
does not inherit its process-first backend shape, direct disk access, or
`file://`-only assumptions.

## Planned Crates

- `ton-language-server-core`
  - Platform-neutral language service engine.
  - Owns document state, workspace model, parsing/indexing, diagnostics, and
    editor features.
  - No `tower-lsp`, no native sockets, no direct `std::fs` reads in feature
    implementation, no browser APIs.

- `ton-language-server-native`
  - Native adapter around the core.
  - Owns LSP transport for CLI/editor use, disk-backed workspace access, Acton
    project discovery, and loading `.acton` resources.

- `ton-language-server-wasm`
  - WASM adapter around the core.
  - Owns `wasm-bindgen` exports, worker message handling, browser workspace
    snapshots, and serialization for Monaco clients.

The first implementation slice starts with only `ton-language-server-core`.
Native and WASM adapters remain planned sibling crates.

## Design Principles

- The core is a library, not a process.
- LSP is an adapter protocol, not the internal architecture.
- URI handling must be platform-neutral from the start.
- Open documents are authoritative over backing storage.
- Parsing uses the existing `tree-sitter` grammars and `*-syntax` crates. They
  are already available, platform-neutral, and should be reused instead of
  introducing a new parser layer.
- The core should dispatch by language through a small plugin contract. It
  should not grow one hard-coded branch per language per feature.
- Disk, browser storage, generated source bundles, and stdlib bundles are all
  workspace providers behind one core-facing interface.
- Browser support must not require a server process.
- Native support must not require a browser-specific abstraction leak.
- The first vertical slice should be small but real: TL-B `go to definition`
  inside a single file. Multi-file languages and import indexing are explicitly
  out of scope for the first version.
- Performance is a first-class design constraint. Most editor operations should
  feel instant to the user, and the core should make slow paths visible through
  optional profiling.

## Core Responsibilities

The core should eventually own:

- workspace document lifecycle;
- virtual file identity and URI normalization;
- import resolution and path mappings;
- parser cache and incremental document updates;
- project index;
- symbol index;
- definition, references, document symbols, workspace symbols;
- diagnostics;
- hover;
- completions;
- semantic tokens;
- formatting hooks if they can stay platform-neutral;
- cancellation/versioning semantics for editor requests.

The first implementation should own only the subset needed for TL-B:

- single-document lifecycle;
- TL-B parsing;
- single-file symbol index;
- TL-B `go to definition`;
- syntax diagnostics if they fall out naturally from parser errors.

The core should not own:

- process lifecycle;
- stdio, TCP, WebSocket, or worker transport;
- Acton CLI argument parsing;
- Monaco APIs;
- browser persistence APIs;
- direct project discovery from the current working directory.

## Workspace Model

The core needs a host-provided workspace snapshot/source provider.

Required concepts:

- stable document id;
- original URI string;
- normalized logical path;
- language id;
- optional version;
- open text overlay;
- readonly bundled files such as stdlib;
- mutable project files;
- import mappings;
- project settings relevant to analysis.

Native can map logical paths to real filesystem paths. Browser can map logical
paths to Monaco model URIs, uploaded folders, OPFS entries, or ActonScan source
bundles.

## URI and Path Rules

The core must not require `file://`.

Supported schemes should include at least:

- `file://` for native editors;
- `acton://` or another stable logical scheme for Acton-provided bundles;
- `monaco://` or `inmemory://` style model URIs for browser-only documents.

The core should compare files by normalized logical identity, not by raw URI
string and not by OS-canonicalized `PathBuf`.

## Internal API Shape

The core API should expose language-service operations directly. Adapters can
translate LSP requests into these operations.

Initial operations:

- initialize workspace;
- open document;
- change document;
- close document;
- request diagnostics;
- request definition;
- request document symbols.
- apply range-based document edits using LSP/Monaco UTF-16 positions and
  tree-sitter `InputEdit`;

Later operations:

- references;
- hover;
- completion;
- semantic tokens;
- code actions;
- inlay hints;
- formatting.

The core can use LSP-compatible concepts where useful, but should avoid making
`lsp-types` the only internal representation if that makes browser/native or
testing boundaries harder.

## Multi-Language Architecture

The core should support languages through a registry of language plugins. The
registry is part of core; native and WASM adapters only select documents and
forward requests.

Each language plugin owns language-specific knowledge:

- file extensions and language ids;
- parser entry point from the matching `*-syntax` crate;
- typed syntax wrapper used by that language;
- per-document symbol index shape;
- optional feature handlers such as definition, references, hover, completion,
  semantic tokens, and diagnostics.

The core owns language-neutral mechanics:

- document lifecycle and versioning;
- URI/logical path normalization;
- parse cache invalidation;
- request dispatch;
- position/range conversion helpers;
- cancellation/version checks;
- workspace services shared by languages once multi-file support is needed.

The plugin contract should stay small and capability-driven. A language should
not have to implement empty methods for unsupported features. The practical
shape is:

- `parse(document, old_tree) -> parsed_document`;
- `capabilities() -> language_feature_set`;
- optional `document_symbols`;
- optional `definition`;
- optional `references`;
- optional `diagnostics`;
- optional `hover`;
- optional `completion`;
- optional `semantic_tokens`.

Parsed documents should be type-erased at the core cache boundary. The core
should not inspect concrete AST types like `tlb_syntax::SourceFile` or
`tolk_syntax::SourceFile`; it should pass the cached parsed document back to the
owning plugin. This avoids a central enum that must be edited for every
language and keeps language-specific code in language modules.

Do not over-abstract the AST layer. The existing syntax crates have similar but
not identical APIs, and forcing a large common AST trait would create more code
than it removes. Shared helpers should be added only for genuinely repeated
operations, such as:

- converting tree-sitter byte ranges to editor ranges;
- finding a node at a position;
- syntax error collection;
- token text extraction;
- stable symbol id construction.

Each new language should be added by following the same narrow path:

1. Create a language module inside core.
2. Register language id, file extensions, and parser.
3. Build the smallest useful per-document index for that language.
4. Implement one feature end to end.
5. Add in-memory tests that use non-`file://` URIs.

For TL-B, the first plugin remains single-document only. For Tolk and other
multi-file languages, the plugin can later opt into shared project indexing
services once that model is deliberately designed.

## IntelliJ-Inspired Model

The architecture should be closer to the IntelliJ language model than to a set
of standalone LSP request handlers. We should borrow the useful shape, not clone
the whole IntelliJ Platform.

Useful concepts to mirror:

- virtual workspace files;
- open document text and versions;
- parsed syntax trees;
- PSI-like language objects over syntax nodes;
- references as first-class objects;
- resolve as a service over local scopes and indexes;
- lightweight declaration indexes;
- feature providers built on top of PSI/reference/resolve services.

Suggested mapping:

- `Workspace` is the equivalent of IntelliJ project/virtual file access. It
  knows documents by URI/logical path, but does not know language semantics.
- `Document` is the current text, version, language id, and URI.
- `ParsedDocument` is the cached tree-sitter parse result plus language-owned
  typed syntax wrapper from a `*-syntax` crate.
- `PsiFile` is a language-owned facade over `ParsedDocument`. It exposes
  semantic nodes such as declarations, type references, fields, and identifiers.
- `Reference` is a language-owned object created from a position or syntax
  node. It knows how to resolve itself using the current file scope and, later,
  project indexes.
- `SymbolIndex` starts as a per-file declaration index. Later it can grow into a
  project index for languages with imports.
- LSP features are adapters over these concepts. For example,
  `textDocument/definition` becomes: document -> parsed document -> PSI file ->
  reference at position -> resolve -> target range.

This gives us a stable extension path:

1. Add parser and `PsiFile` for a language.
2. Add the smallest symbol index that language needs.
3. Add `Reference` objects for syntax nodes that can resolve.
4. Implement LSP features by composing those pieces.

The first TL-B implementation should follow this model even though it is
single-file. That keeps the first feature small while making the future Tolk
implementation look natural: Tolk can add project indexing and import-aware
resolve without changing how LSP handlers are structured.

Things not to copy from IntelliJ in the first version:

- global application/project service containers;
- read/write action machinery;
- extension-point infrastructure;
- stub indexes before we need cross-file lookup;
- a universal AST/PSI trait that every language must fully implement.

## Performance and Profiling

The language server should be designed for interactive latency. The target is
not just correctness; common editor operations should usually complete within a
single frame or feel instantaneous.

Performance principles:

- Do parsing and indexing on document changes, not repeatedly on every feature
  request.
- Keep request handlers mostly as cache lookups plus small local computation.
- Reuse parse trees through tree-sitter incremental parsing.
- Use incremental parsing only for range-based edits where the core can build a
  correct `InputEdit`; full-text replacement is a safe fallback parse path.
- Keep per-document indexes compact and cheap to rebuild for single-file
  languages.
- Make cross-file/project indexing explicit and incremental when it is added
  later.
- Avoid global locks on request paths.
- Avoid cloning full document text or large indexes in feature handlers.
- Keep WASM/browser costs in mind: serialization, copying strings across the
  JS/WASM boundary, and worker message size are part of latency.
- Keep tree-sitter WASM builds on the upstream path: `tree-sitter` 0.26+
  consumes the `tree-sitter-language` WASM headers/sources, and grammar crates
  should expose that crate to their build scripts instead of vendoring local
  libc shims or generated workaround files.

Profiling should be optional and cheap when disabled.

Planned profiling shape:

- feature-gated or runtime-disabled instrumentation in core;
- spans/timers for document open/change, parse, index rebuild, diagnostics,
  definition, references, completion, semantic tokens, and adapter boundary
  conversion;
- counters for cache hits/misses, parsed bytes, indexed symbols, and request
  counts;
- adapter-specific reporting hooks:
  - native can log profiling summaries;
  - WASM can expose profiling events or summary snapshots to the host page;
- tests or benchmarks can assert broad performance budgets for representative
  fixtures without making normal tests flaky.

Initial performance expectations:

- TL-B single-file `go to definition` should be a cache lookup plus local
  resolve after the document has been parsed and indexed.
- Opening/changing a small TL-B file should parse and rebuild the per-file index
  immediately enough for interactive editing.
- Definition requests should not parse the document if the cached version is
  current.
- Rendering/test formatting should stay outside measured core feature timings.

## Logging Strategy

Logging and profiling should be related but separate:

- logging explains what the language service is doing;
- profiling measures how long specific operations take.

The core should use structured logging through `tracing` spans and events. It
should not initialize a global logger, write to stdout/stderr, write files, or
call browser console APIs. Native and WASM adapters own the logging sink and the
active filter level.

Initial core support includes public `LogLevel` and `LoggingConfig` helpers for
adapter-owned filters, stable target constants, and structured events around
document lifecycle, range edits, TL-B parsing/indexing, and TL-B definition
resolution.

Recommended level semantics:

- `error`
  - unexpected internal failures and adapter setup failures;
  - should be rare in core because most recoverable problems are returned as
    typed errors or diagnostics.
- `warn`
  - recoverable service-level problems, such as unsupported language ids,
    failed workspace provider reads, stale document versions, or failed
    incremental reparse fallback;
  - parser syntax errors should usually become diagnostics, not warning logs.
- `info`
  - lifecycle summaries: initialize, open/change/close document, request start
    and finish, language registration, workspace provider changes;
  - keep this useful for normal debugging without being noisy.
- `debug`
  - cache hits/misses, parse mode, index rebuild summaries, resolve path,
    candidate counts, selected target counts, diagnostics counts.
- `trace`
  - detailed edit conversion, tree-sitter byte/point ranges, PSI node kinds,
    scope walk details, resolve candidate lists.

Every operation log should include enough structured fields to correlate events:

- operation name;
- request id if the adapter has one;
- URI/logical document id;
- language id;
- document version;
- edit count or change kind;
- result count;
- whether an incremental tree was reused;
- broad cache hit/miss/fallback flags.

The core should avoid logging full source text, token contents, or large
serialized AST/debug dumps by default. Trace-level logs may include short symbol
names and node kinds, but not whole files. Adapters can optionally redact or
normalize URI paths when logs may leave the local machine.

Configuring the active log level should be adapter-owned:

- native adapter:
  - read an initial level from CLI flags, environment, or LSP
    `initializationOptions`;
  - support dynamic updates through `workspace/didChangeConfiguration` using a
    reloadable `tracing_subscriber` filter;
  - optionally write to stderr, rotating files, or editor LSP log messages.
- WASM adapter:
  - expose a worker command such as `setLogLevel(level, target_filter)`;
  - route logs to browser console or an in-page debug panel;
  - keep filtering inside the worker so trace-level logging does not flood the
    main thread unless explicitly enabled.

Suggested target hierarchy:

- `ton_language_server_core::service` for document lifecycle and feature
  dispatch;
- `ton_language_server_core::edit` for range edit and `InputEdit` conversion;
- `ton_language_server_core::languages::<language>` for language-specific parse,
  index, PSI, and resolve events;
- `ton_language_server_native::*` and `ton_language_server_wasm::*` for adapter
  transport and host integration.

For testing, use a test subscriber in integration tests rather than asserting on
stdout. Tests should verify that:

- level filtering can hide or expose debug/trace events;
- common operations emit stable structured event names;
- trace-level edit logs are available when debugging invalid incremental trees;
- logging remains disabled or filtered cheaply on hot paths.
- file-based log snapshots are rendered through an explicit redaction layer
  before comparison, so local filesystem paths, workspace roots, and user-owned
  URIs do not become stable test fixtures.

## Testing Strategy

The legacy self-contained tests had the right user-facing shape and
should heavily influence the new test harness:

- inline source snippets;
- caret markers such as `<caret>`;
- feature-specific case helpers;
- rendered, human-readable outputs;
- `expect-test` snapshots for resolved targets, references, completions, hover,
  semantic tokens, and similar behavior.

Snapshot tests should use two formats:

- inline `expect-test` snapshots for small, highly local cases where the source
  snippet and expected output fit comfortably in one test;
- file-based snapshots/fixtures for larger language-server scenarios, following
  the Acton-style approach used by existing repository tests: test files contain
  named cases, source snippets, properties such as `only`, and expected output
  blocks that can be refreshed with an explicit snapshot update mode such as
  `UPDATE_SNAPSHOTS`.

File snapshots should be preferred for LS outputs that naturally grow over time:
diagnostics, semantic tokens, document symbols, completion lists, hover text,
references, multi-file workspace cases, logging/profiling summaries, and adapter
request/response rendering. This keeps test reviews readable and avoids large
inline blobs in Rust files.

The new architecture should keep those strengths but improve the layering.
Most core tests should not spin up an LSP server and should not depend on
`tower-lsp`. They should call the core API directly.

Recommended layers:

- **Core unit tests**
  - Test URI/logical path normalization, UTF-16 position conversion, text edit
    application, document versioning, and parse-cache invalidation.
  - These should use direct assertions when checking small scalar behavior.

- **Performance tests and benchmarks**
  - Add focused benchmarks for parse, index rebuild, and hot feature requests.
  - Keep benchmarks separate from correctness snapshots.
  - Use profiling output to diagnose regressions before adding new abstraction.

- **Language plugin tests**
  - Test each language through the plugin contract, not through LSP transport.
  - Use marked source snippets and snapshots for PSI/reference/resolve output.
  - For TL-B, start with single-file definition cases.
  - Include non-`file://` URIs in every feature group from the start.
  - Keep language-specific integration tests under `tests/languages/<language>/`
    and register them as explicit Cargo test targets. Top-level `tests/` should
    stay reserved for core-wide service, logging, profiling, and adapter harness
    tests.

- **PSI and index tests**
  - Snapshot the language-owned semantic view: declarations, references, symbol
    ids, and resolved targets.
  - Keep these below LSP so failures point at the language model rather than
    adapter formatting.

- **Core feature tests**
  - Exercise `LanguageService` operations such as `definition`,
    `document_symbols`, and `diagnostics`.
  - Render results in a stable text format compatible with existing snapshots
    `render_resolve`.

- **Adapter tests**
  - Native and WASM adapters should have thin smoke tests proving request/response
    conversion, initialization, and document lifecycle.
  - They should not duplicate every language semantic case.

- **WASM/browser tests**
  - Verify the core compiles for `wasm32-unknown-unknown`.
  - Add a small worker-level smoke test once the WASM adapter exists.
  - Keep Monaco-specific tests focused on wiring, not semantic correctness.

Improvements over the old harness:

- Separate fixture parsing from LSP request construction so core tests can reuse
  marked snippets without creating LSP params.
- Support multiple named markers, for example `<caret>`, `<target>`,
  `<target:foo>`, to make expected ranges clearer.
- Snapshot semantic model output before snapshotting LSP-shaped output.
- Make URI scheme part of test cases. Every language feature should have at
  least one `file://` and one virtual URI case.
- Keep language harness helpers small and parallel: `case_tlb_definition`,
  `case_tlb_references`, etc.
- Use snapshots for scenario-style behavior and larger feature outputs. Reserve
  plain assertions for small invariants.
- Store large LS scenario expectations in fixture files rather than inline Rust
  snapshots, and make snapshot update explicit so CI never rewrites fixtures.

Initial TL-B test matrix:

- constructor/type reference resolves to one declaration;
- reference resolves to multiple declarations with the same resulting type;
- unresolved reference renders as unresolved;
- field/type-parameter references resolve inside the same file;
- parser syntax errors produce stable diagnostics if diagnostics are included in
  the first slice;
- same cases pass with a virtual URI such as `acton://fixture/main.tlb`.

## Milestones

### 0. Planning and Boundaries

- Create this plan.
- Agree crate names and scope.
- Decide whether `ton-language-server-core` starts as a workspace member now or
  only after the first Rust module is ready.

### 1. Core Skeleton

- Add `ton-language-server-core` as a workspace crate.
- Define public module boundaries.
- Add minimal domain types for URI, positions, ranges, diagnostics, and edits.
- Add `LanguageId`, language registry, feature-set/capability model, and the
  minimal plugin contract.
- Add full-document replacement and range-based edit operations. Range edits
  should apply `InputEdit` to the previous tree before reparsing.
- Define the IntelliJ-inspired vocabulary in code: workspace/document,
  parsed-document cache, PSI facade boundary, reference/resolve boundary, and
  symbol index boundary.
- Add optional profiling hooks with no meaningful overhead when disabled.
- Add a reusable core test harness for marked snippets, marker positions, and
  stable rendering.
- Add tests for URI/path normalization.

### 2. Workspace Host

- Add in-memory workspace provider for tests and browser use.
- Add disk workspace provider in the native crate, not in core.
- Model stdlib and generated files as readonly workspace layers.
- Add tests for open-document overlay precedence.
- Keep multi-file project graph construction out of the first implementation.
  The first feature set only requires a single open TL-B document.

### 3. Parser and File Index

- Integrate TL-B parsing through `tree-sitter` and the existing `tlb-syntax`
  crate.
- Keep the parser boundary compatible with the other existing `*-syntax` crates
  (`tolk-syntax`, `fift-syntax`, `tasm-syntax`, `toml-syntax`, and
  `ton-syntax`) so later language features can reuse the same core shape.
- Cache parse results by document id and version.
- Reuse edited tree-sitter trees for incremental range edits, and verify them
  against clean parses in tests.
- Build a single-document TL-B symbol index.
- Add a TL-B PSI-like facade and TL-B reference objects instead of implementing
  definition directly from raw LSP positions.
- Add profiling spans for parse and index rebuild.
- Register TL-B as the first language plugin rather than special-casing TL-B in
  core request handlers.
- Add tests using in-memory files only.

### 4. First Feature: TL-B Go To Definition

- Implement TL-B go-to-definition for declarations and references inside one
  file.
- Route definition through the IntelliJ-like pipeline: position -> PSI/reference
  -> resolve -> target range.
- Do not implement imports, mapped imports, stdlib indexing, or cross-file
  lookup in the first version.
- Return target URI and range without requiring a native filesystem path.
- Add profiling for definition requests and confirm hot requests do not reparse
  unchanged documents.
- Add snapshot tests using marked TL-B snippets, including browser-relevant
  non-`file://` URIs.

### 5. Diagnostics

- Wire syntax diagnostics from parser results.
- Reuse or adapt linter/type infrastructure only after its filesystem coupling
  is isolated.
- Keep diagnostics version-aware.

### 6. Native Adapter

- Create `ton-language-server-native`.
- Connect the core to a native LSP transport.
- Load Acton project configuration and stdlib.
- Keep compatibility with the existing `acton ls` command surface later, after
  the new native adapter is usable.

### 7. WASM Adapter

- Create `ton-language-server-wasm`.
- Export worker-friendly entry points.
- Add bundling/build workflow for the UI package.
- Verify the crate compiles for `wasm32-unknown-unknown`.
- Keep all browser APIs outside core.

### 8. Monaco Integration

- Add a worker wrapper that speaks LSP-style JSON-RPC or a thin local protocol.
- Register Monaco providers through `monaco-languageclient` or direct Monaco
  providers, depending on which keeps less glue code.
- Start with go-to-definition and diagnostics.
- Add multi-file browser workspace loading.

### 9. Migration from the Legacy Language Server

- Compare feature parity with the legacy implementation.
- Move missing language features into the new core incrementally.
- Add Tolk only after the core has a deliberate project indexing model for
  languages with imports and multiple files. See
  [`TOLK_DESIGN.md`](TOLK_DESIGN.md) for the proposed Tolk workspace analysis
  model. The first Tolk slice should stop at resolver-backed features such as
  go to definition and references; type inference, lints, completion, and
  type-aware hover come later.
- Native, VS Code, WASM, and Monaco users now run the shared core.
- The legacy crate was deleted after replacement coverage was established.
- Tolk diagnostics remain a separate future slice and are not part of the
  completed migration scope.

## Open Decisions

- Should core domain types wrap `lsp-types`, convert to/from them, or avoid them
  entirely?
- What logical URI scheme should Acton-owned browser bundles use?
- Should browser Monaco integration use `monaco-languageclient` from day one or
  direct Monaco providers for the first slice?
- Should stdlib be embedded into the WASM bundle, loaded as a separate asset, or
  provided by the host page?
- How much of `tolk-resolver` should be reused versus rewritten around the new
  workspace model?
- Should future language plugins live inside `ton-language-server-core`, or
  should large languages move to sibling crates once the plugin contract is
  stable? The first implementation should keep TL-B inside core to avoid
  premature crate boundaries.

## Initial Acceptance Criteria

- The core can be tested without filesystem access.
- The same definition request works with `file://` and non-`file://` URIs.
- The first browser integration can run with no backend process.
- Native and browser adapters do not duplicate language analysis logic.
- Adding the next single-file language should require registering one language
  plugin and adding language-local code, not editing every request handler.
- TL-B definition behavior is covered by snapshot tests that do not start an LSP
  server.
- Profiling can be enabled to inspect parse/index/request timings in both native
  and WASM adapters.
- The legacy crate can be removed without changing native or browser behavior.
