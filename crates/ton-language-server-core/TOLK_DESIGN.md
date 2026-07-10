# Tolk Language Server Design

## Goal

Add Tolk support to the new language server architecture without inheriting any
legacy language-server API shape. The design is based on the current Tolk
engine crates:

- `tolk-syntax` for tree-sitter parsing and typed syntax nodes;
- `tolk-resolver` for file indexing, import graph construction, symbols, and
  name resolution;
- later `tolk-ty`, `tolk-analysis`, and `tolk-linter` for type-aware and
  diagnostic features after resolver support is stable.

The same core must work in native local editor integration and in browser/wasm
integration. Browser support means local execution in a Web Worker with
host-provided files, not a backend service.

## Non-Goals

- Do not reuse or mirror the legacy language-server backend API.
- Do not make `ton-language-server-core` depend on a native filesystem.
- Do not run project-wide analysis directly inside a feature request handler.
- Do not make hover, definition, references, or completion mutate analysis
  state.
- Do not introduce a universal AST abstraction that all languages must share.
- Do not require Tolk to fit into the single-document TL-B plugin model.
- Do not include type inference, lint diagnostics, code actions, completion, or
  type-aware hover in the first Tolk version.

## Legacy Behavior Reference

Ported Rust tests and upstream fixtures preserve relevant behavior, edge cases,
and rendering formats from the removed implementation. That behavior does not
define the public API, ownership model, threading model, workspace model, or
native/browser boundary of the new implementation.

Useful things to preserve from legacy behavior:

- feature behavior and user-visible edge cases;
- test case shape and snapshot rendering ideas;
- small algorithms that can be moved behind the new workspace/snapshot model;
- compatibility details where existing users already rely on behavior.

Things not to borrow:

- process-first backend shape;
- direct filesystem assumptions inside language features;
- request handlers that rebuild semantic state;
- global mutable state as the primary analysis API;
- `file://`-only identity assumptions.

## Version Scope

The first Tolk version should stop at resolver support.

V1 includes:

- platform-neutral source provider and open-document overlay;
- root and import loading;
- project index construction;
- name resolution across files;
- immutable resolve snapshot;
- go to definition;
- references;
- resolver-focused logs, profiling, and snapshot tests.

V1 explicitly excludes:

- type inference;
- dataflow analysis;
- linter diagnostics;
- code actions;
- completion;
- type-aware hover;
- semantic tokens.

Those features should be added only after the resolver snapshot is reliable in
native and wasm environments.

## Current Engine Shape

The useful existing Tolk pipeline is already split across reusable crates:

- [`FileDb`](../tolk-resolver/src/file_db.rs) stores parsed files, stable
  `FileId`s, line offsets, source kind, typed syntax roots, and per-file
  `FileIndex` values.
- [`ProjectIndex`](../tolk-resolver/src/project_index.rs) stores files, imports,
  dependents, path-to-file mappings, global symbols, and resolved uses.
- [`FileResolveIndex`](../tolk-resolver/src/resolve_index.rs) stores local
  definitions and name-use resolution for one file.
- [`TypeDb`](../tolk-ty/src/type_db.rs) is a short-lived analysis object that
  borrows `FileDb`, `ProjectIndex`, and `TypeInterner`.
- [`infer`](../tolk-ty/src/type_inference.rs) computes body inference results
  for top-level declarations.
- [`AnalysisDb`](../tolk-analysis/src/lib.rs) lazily computes dataflow facts and
  control-flow graphs over resolver/type information.
- [`Checker`](../tolk-linter/src/lib.rs) consumes `FileDb`, `TypeDb`, body
  inference results, and `AnalysisDb` to produce diagnostics.

The language server should compose this pipeline rather than reimplementing
resolver, type inference, or lint rules.

For V1, only the `tolk-syntax` and `tolk-resolver` parts are on the critical
path. The type, analysis, and linter crates are reserved for the next semantic
layer.

## Core Architecture

Tolk needs a workspace-aware language engine, not only a document plugin.

The language-server core should gain an optional workspace-language layer. The
existing document-local plugin remains enough for TL-B, TASM, and Fift features
that only need the current parsed file. Tolk opts into the heavier workspace
path.

Recommended shape:

```rust
pub trait WorkspaceLanguage: Send + Sync {
    fn did_open(&self, document: &DocumentSnapshot, parsed: &dyn ParsedDocument);
    fn did_change(&self, document: &DocumentSnapshot, parsed: &dyn ParsedDocument);
    fn did_close(&self, uri: &DocumentUri);
    fn latest_snapshot(&self) -> Option<Arc<dyn Any + Send + Sync>>;
}
```

This trait is intentionally small. It is lifecycle and snapshot plumbing, not a
second LSP API. Feature providers still live on the language side, but they read
from the latest immutable Tolk snapshot instead of rebuilding analysis.

Tolk-specific state should live in a dedicated `TolkWorkspaceEngine`:

- open document overlay;
- source provider;
- root-file model;
- import mappings;
- current `FileDb`;
- latest immutable analysis snapshot;
- analysis scheduler state;
- snapshot publication state.

## Source Provider

The first hard boundary is file access. Tolk imports cannot be tied to
`std::fs`, because browser integration must load files from memory, Monaco
models, uploaded folders, or fetched static assets.

Introduce a platform-neutral source provider:

```rust
pub trait TolkSourceProvider: Send + Sync {
    fn read(&self, path: &TolkPath) -> anyhow::Result<Option<Arc<str>>>;
    fn exists(&self, path: &TolkPath) -> bool;
    fn canonicalize(&self, path: &TolkPath) -> anyhow::Result<TolkPath>;
}
```

`TolkPath` should be a normalized logical path, not necessarily an OS path. A
native adapter can map it to `PathBuf`. A browser adapter can map it to virtual
URIs or in-memory names.

Provider layers:

- open-document overlay, highest priority;
- mutable workspace files;
- readonly stdlib files;
- readonly generated or host-provided files;
- optional fetched browser assets.

`FileDb::process_content_incremental` already accepts text and an old tree, so
it is a good entry point for open buffers. The remaining native coupling is
project/import loading, where builder code should use the provider rather than
direct disk reads.

## Snapshots

Feature handlers should read immutable snapshots. V1 needs only a resolve
snapshot:

```rust
pub struct TolkResolveSnapshot {
    generation: u64,
    roots: Vec<TolkPath>,
    file_versions: HashMap<FileId, DocumentVersion>,
    files: Arc<TolkFileSnapshotSet>,
    project_index: Arc<ProjectIndex>,
    project_errors: Arc<Vec<TolkProjectError>>,
    timing: AnalysisTiming,
}
```

`files` represents the immutable file data needed by request handlers: text,
line offsets, syntax roots, and stable file identities. It can initially wrap
the existing `FileDb` if access is disciplined as immutable during snapshot
reads, but the public request path should not depend on mutable file database
state.

A later semantic snapshot can layer on top of this:

```rust
pub struct TolkSemanticSnapshot {
    resolve: Arc<TolkResolveSnapshot>,
    body_types: Arc<HashMap<FileId, HashMap<SymbolId, InferenceResult>>>,
    diagnostics: Arc<HashMap<FileId, Vec<TolkDiagnostic>>>,
    timing: SemanticAnalysisTiming,
}
```

`TypeDb` should still not be stored directly in the semantic snapshot because it
borrows `FileDb`, `ProjectIndex`, and `TypeInterner`. Store the durable
inputs/results instead. When rendering type information later, construct a
short-lived `TypeDb` against snapshot-owned data.

The snapshot must be atomically replaceable:

- request handlers read the latest complete snapshot;
- analysis workers build the next snapshot off the request path;
- stale analysis results are discarded by generation/version checks.

## Analysis Pipeline

The V1 resolver pipeline has explicit phases:

1. **Parse and file index**
   - Parse changed open files with tree-sitter incremental parsing when a valid
     edited tree is available.
   - Build/update per-file `FileIndex`.
   - Record parser errors in the file snapshot so future diagnostics can expose
     them.

2. **Project index**
   - Determine root files.
   - Resolve imports through the source provider and mappings.
   - Build `ProjectIndex`, including imports, dependents, path mapping, and
     global symbols.
   - Record import/project errors in the resolve snapshot.

3. **Name resolution**
   - Run resolver over the project index.
   - Populate `resolved_uses`.
   - This enables definition and references.

4. **Publish**
   - Replace the latest resolve snapshot.
   - Update profiling data.

Semantic phases are future work:

1. **Type inference**
   - Create a temporary `TypeDb`.
   - Collect top-level types.
   - Run body inference per top-level declaration.
   - Store `body_types` in the semantic snapshot.

2. **Analysis facts**
   - Use `AnalysisDb` lazily while running lints.
   - Keep the facts inside the analysis job or snapshot only if they are needed
     by multiple request types.

3. **Linter diagnostics**
   - Run `Checker` over workspace files.
   - Apply suppressions and settings.
   - Convert diagnostics and fixes to LS-native diagnostics/code actions.

4. **Publish**
   - Replace the latest semantic snapshot.
   - Publish diagnostics for affected files.
   - Update profiling data.

## Root and Import Model

Tolk projects are multi-file. The LS should not pretend that every open file is
an isolated program.

Root selection should be host-configurable:

- native adapter discovers Acton project roots and contract/script/test roots;
- browser adapter receives roots from the page or project bundle manifest;
- fallback mode treats the active file as a temporary root.

The project index should keep both directions:

- imports from a file to its dependencies;
- dependents from a file to files that include it.

This lets the scheduler decide affected roots after an edit. The first version
can rebuild all roots for simplicity, but the data model should already expose
dependents so incremental invalidation is possible.

## Feature Implementation

### Go To Definition

Definition should be a snapshot lookup:

1. Convert UTF-16 position to byte offset using the document text index.
2. Find the file id for the document.
3. Ask `ProjectIndex::find_use` or `FileResolveIndex::find_local_at`.
4. Resolve:
   - local definition to local span in the same file;
   - global symbol to `ProjectIndex::resolve_symbol`;
   - unresolved name to no result.
5. Convert byte span to LS range using the target file's line offsets.

No parsing, import resolution, or type inference should run during this request.

### References

References are also resolver-backed:

- local references come from the current file's `local_usages_of`;
- global references scan all `resolved_uses` in the snapshot;
- declaration span should be included when requested by the caller.

The first version can restrict global references to files reachable from the
current root. Workspace-wide references can be added once multi-root behavior is
stable.

### Hover

Hover is not part of V1. When added, it should layer information by cost:

1. symbol/local declaration text and kind from resolver/index data;
2. signature/type from `body_types` and a short-lived `TypeDb`;
3. diagnostics or documentation if available.

If type data is stale or missing, hover should still return resolver-level
information instead of blocking for analysis.

### Completion

Completion is not part of V1. It should be designed after
definition/references are stable, using:

- local scopes from `FileResolveIndex`;
- visible global symbols from `ProjectIndex`;
- import context from syntax;
- type context from `TypeDb` and inference when available.

Completion can tolerate stale type information, but it must not block the UI on
a full project rebuild.

### Diagnostics and Code Actions

Full Tolk diagnostics and code actions are not part of V1. When added, they
should preserve engine diagnostics rather than flattening them too early:

- primary annotation becomes the main diagnostic range;
- secondary annotations become related information;
- severity maps from linter severity/settings;
- `Fix` values become code actions;
- project/import errors become diagnostics tied to import spans when possible.

Code actions can be added after diagnostics are stable, but the diagnostic
conversion should retain enough fix identity to avoid redesigning it later.

## Scheduling and Cancellation

Editor operations have different latency budgets:

- document parse and file indexing should happen immediately;
- resolve snapshot should update after a short debounce;
- future type inference and lint diagnostics can run after the same debounce or
  a slightly longer idle window;
- expensive future features should be cancellable by generation.

Every analysis job should capture:

- generation id;
- root set;
- file versions;
- settings version;
- source provider revision.

The result is accepted only if it still matches the latest service state.

## Logging and Profiling

The Tolk engine should use the existing core logging/profiling direction:

- `tracing` targets under `ton_language_server_core::languages::tolk`;
- no global logger initialization inside core;
- no stdout/stderr/browser console writes from core;
- runtime log-level control remains adapter-owned;
- V1 profiling timers for parse, file index, project index, resolve, snapshot
  publish, definition, and references.

Future semantic profiling should add timers for type inference, lint, hover,
completion, and code actions.

Profiling should distinguish host costs from engine costs:

- provider read time;
- parse/index time;
- resolve time;
- future type/lint time;
- request handler time;
- adapter serialization time, if reported by native/wasm adapters.

The browser UI can show the latest snapshot timing and live request timings, but
those are presentation concerns outside core.

## Testing Strategy

Tolk LS tests should live under:

```text
crates/ton-language-server-core/tests/languages/tolk/
```

Use snapshots for behavior that humans need to inspect:

- definition targets;
- references;
- project/import errors;
- logs after redaction;
- profiling summaries.

Recommended fixture shape:

- multi-file fixture cases with named files;
- markers such as `<caret>`, `<target>`, and `<target:name>`;
- configurable URI scheme per case;
- optional project roots and import mappings;
- expected output blocks rendered in stable text form.

Required test groups:

- single-file definition and references;
- cross-file import definition and references;
- unresolved imports and unresolved names;
- open-document overlay has priority over provider-backed files;
- virtual URI/browser provider cases;
- incremental edit keeps a valid tree and updates affected analysis;
- profiling captures phase timings when enabled;
- logging snapshots pass through path/URI redaction before comparison.

The tests should call core APIs directly. Native LSP and browser/Monaco tests
should be smoke tests for transport/wiring, not semantic duplicates.

Future semantic test groups should cover diagnostics, fixes/code actions,
hover, completion, type inference, dataflow, and linter integration.

## Implementation Phases

### 1. Workspace Hook

- Extend the language model with optional workspace-language lifecycle hooks.
- Keep existing document-local languages working unchanged.
- Add tests that a no-op workspace language receives open/change/close events.

### 2. Source Provider Boundary

- Introduce logical Tolk paths.
- Add in-memory source provider for tests and wasm.
- Add overlay precedence tests.
- Refactor project/import loading to read through the provider.

### 3. Tolk Workspace Engine Skeleton

- Add `TolkWorkspaceEngine`.
- Track roots, open overlays, mappings, and generation.
- Build parse/file index for open documents.
- Record parser errors in file snapshots for future diagnostics.

### 4. Project Snapshot

- Build `ProjectIndex` from provider-backed roots.
- Run name resolution.
- Publish immutable snapshots.
- Implement definition and references from snapshots.

### 5. Resolver V1 Hardening

- Use dependents to limit affected roots.
- Cache reusable project data carefully.
- Add resolver-focused performance benchmarks and profiling snapshots.
- Verify wasm request latency and transfer sizes.

### 6. Future Semantic Snapshot

- Run type inference and collect `body_types`.
- Run linter diagnostics.
- Convert diagnostics to core diagnostics.
- Preserve fixes for future code actions.

### 7. Future Hover and Completion

- Add hover from resolver/type snapshot.
- Design completion around resolver scopes and type context.
- Keep stale/missing type information graceful.

## Open Decisions

- Should Tolk language support stay inside `ton-language-server-core`, or move
  to a sibling crate once the workspace-language API stabilizes?
- Should the first project snapshot rebuild all roots, or should it immediately
  use dependents for affected-root analysis?
- What exact logical URI/path scheme should browser Acton project bundles use?
- How should Acton config settings be represented in browser mode when there is
  no project directory?
- Should V1 surface import/project errors through diagnostics immediately, or
  keep them only in resolver snapshots until full diagnostics are implemented?
- Should `AnalysisDb` results be stored in snapshots for future features, or
  kept as linter-job-local cache until another feature needs them?

## Acceptance Criteria

- Tolk support works with no native filesystem access in core.
- Native and wasm adapters share the same Tolk analysis implementation.
- Definition and references are resolver-backed and do not reparse files.
- Open buffers override provider-backed file contents.
- Non-`file://` URIs are covered by tests from the first Tolk feature.
- Profiling can show where resolver analysis time is spent.
- V1 does not require `tolk-ty`, `tolk-analysis`, or `tolk-linter` on the hot
  path.
- The architecture supports future completion and code actions without replacing
  the workspace model.
