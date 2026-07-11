# Tolk Incremental Analysis Design

## Status

This document defines the performance work required before the Tolk language
server can be considered responsive on large projects. It supersedes the
eager semantic rebuild described by the current implementation while keeping
the source-provider and workspace model from `TOLK_DESIGN.md`.

The implementation must remain shared by native and WebAssembly adapters.
Adapter-specific debouncing is not a substitute for fixing core invalidation.

## Problem Statement

Every open-document change currently performs the following work before the
language server accepts another request:

1. incrementally parse the changed document;
2. update its `FileDb` entry;
3. rebuild the complete `ProjectIndex`;
4. resolve every file in the project;
5. refresh type signatures for affected files;
6. infer every top-level body in each affected file;
7. recompute file-wide use facts;
8. deep-clone semantic state into a new immutable snapshot.

Only the first two operations are intrinsically required for every keystroke.
The other phases use invalidation scopes that are much wider than the semantic
effect of a typical body edit.

This causes latency to grow with both workspace size and edited-file size.
WASM overhead is visible but is not the primary bottleneck: native and WASM
show nearly identical time inside the shared invalidation pipeline.

## Measured Baseline

Measurements use release builds and the same stdio LSP harness for native,
WASM, and the TypeScript implementation. Each reported comparison uses five
process runs with rotated server order.

### Acton Jetton Template

- 59 Tolk files;
- 439 KiB of Tolk source;
- edited file: 6.2 KiB;
- typed expression: `storage.save()` with completion after every character.

| Operation | WASM | Native | TypeScript |
| --- | ---: | ---: | ---: |
| Cold start and indexing | 127.1 ms | 35.7 ms | 255.2 ms |
| Edit to definition | 11.3 ms | 11.3 ms | 1.2 ms |
| Edit and completion per character | 15.3 ms | 14.4 ms | 3.7 ms |
| Typing p95 | 17.3 ms | 16.1 ms | 5.0 ms |

### `acton-contracts`

- 286 Tolk files;
- 1.88 MiB of Tolk source;
- edited file: `elector/contracts/Elector.tolk`, 31.5 KiB;
- typed expression: `storage.save()` with completion after every character.

| Operation | WASM | Native | TypeScript |
| --- | ---: | ---: | ---: |
| Cold start and indexing | 279.9 ms | 116.7 ms | 406.7 ms |
| Edit to definition | 69.6 ms | 63.0 ms | 6.2 ms |
| Edit and completion per character | 84.9 ms | 76.7 ms | 17.0 ms |
| Typing p95 | 93.6 ms | 84.3 ms | 22.2 ms |

The edited file is about five times larger than the Jetton file. Native edit
latency grows by 5.6 times and native typing latency grows by 5.3 times. This
shows that the current hot path is effectively a full-file semantic rebuild.

### Differential Profile

The following profile measures one comment-only edit after `acton-contracts`
has been fully indexed and `Elector.tolk` has been opened:

| Phase | Time | Share of snapshot rebuild |
| --- | ---: | ---: |
| Incremental parse | 2.770 ms | outside snapshot timer |
| Update changed `FileDb` entry | 0.102 ms | 0.1% |
| Rebuild `ProjectIndex` | 3.716 ms | 5.0% |
| Resolve project | 50.478 ms | 67.4% |
| Refresh type signatures | 0.394 ms | 0.5% |
| Infer bodies | 7.017 ms | 9.4% |
| Recompute use facts | 3.833 ms | 5.1% |
| Materialize snapshot | 9.061 ms | 12.1% |
| Total snapshot rebuild | 74.866 ms | 100% |

The first optimization target is project-wide name resolution, not type
inference. Deferring only type inference would leave most of the latency.

## Goals

- A body-only edit must not resolve unaffected files.
- A body-only edit must not infer unaffected declarations.
- Publishing a snapshot must not deep-clone all workspace semantic data.
- Work after a keystroke must be proportional to the changed declaration, not
  to the workspace or complete edited file.
- Completion, hover, definition, references, rename, semantic tokens, inlay
  hints, and code actions must never observe a mixture of generations.
- Native and WASM must execute the same invalidation algorithm.
- Expensive global features may lazily fill generation-scoped caches, but they
  must produce current results before returning.
- All existing Tolk behavior tests remain authoritative.

## Non-Goals

- Do not hide synchronous work with an arbitrary debounce and call it fixed.
- Do not return stale definition or rename results while an edit is pending.
- Do not make WASM use a less correct semantic model than native.
- Do not introduce a second resolver or type engine in the language server.
- Do not optimize diagnostics in this phase; the design only needs to leave a
  correct generation and cancellation boundary for them.

## Change Classification

Each changed file is classified by comparing structured old and new indexes.
Text hashes and ad hoc source matching are insufficient because invalidation
depends on semantic surfaces, not byte equality.

### Body-Only Change

The following remain unchanged:

- imported paths;
- top-level symbol IDs, names, fully qualified names, and resolver kinds;
- nested struct-field and enum-member IDs and names;
- declaration signature syntax relevant to `TypeDb`.

Required work:

- parse and update the changed file;
- resolve the changed file only;
- infer changed top-level declarations only;
- recompute facts only for changed declarations or defer file facts;
- reuse every unaffected file and declaration cache entry.

### Resolver-Surface Change

Examples include adding, deleting, renaming, or reordering declarations;
changing a declaration between value/type kinds; and changing struct fields or
enum members.

Required work:

- resolve the changed file;
- resolve transitive import dependents because their global targets may change;
- refresh type signatures for the same conservative dependent closure;
- invalidate body inference only where signatures or resolved targets changed.

### Import-Graph Change

An import was added, removed, or retargeted.

Required work:

- update import and reverse-dependent edges;
- invalidate the changed file;
- invalidate the union of old and new dependent closures;
- preserve unrelated project components.

### Workspace-Shape Change

Files, roots, mappings, stdlib roots, or workspace configuration changed.

The first implementation may conservatively rebuild the complete project for
these comparatively rare operations. It must still publish state through the
same generation mechanism.

## Target Architecture

### 1. Separate Resolve and Semantic Generations

The workspace owns a monotonically increasing generation containing:

```rust
struct TolkGeneration {
    id: u64,
    resolve: Arc<TolkResolveSnapshot>,
    semantics: TolkSemanticCache,
}
```

The resolve snapshot is published immediately after parse, index update, and
incremental name resolution. Semantic cache entries are tied to the same
generation and cannot be reused without an explicit validity decision.

### 2. Reuse Per-File Resolve Indexes

`ProjectIndex` already stores each `FileResolveIndex` behind an `Arc`, and the
resolver already exposes `resolve_files`. A new project generation must:

1. build or update project topology;
2. compute the invalidated resolver file set;
3. copy `Arc<FileResolveIndex>` values for valid files from the previous
   generation;
4. call `resolve_files` only for invalidated files.

For a body-only edit, the invalidated set contains exactly the changed file.
For a resolver-surface or import-graph change, it contains the changed file and
the conservative dependent closure.

Reused indexes are valid only when:

- the source file itself is unchanged;
- all global symbols it can resolve to retain compatible `SymbolId`s and
  resolver kinds;
- its import targets are unchanged.

The resolver crate should own this validation API. Core should not reach into
`ProjectIndex` fields or duplicate resolver invariants.

### 3. Structured Resolution Surfaces

`FileIndex` needs an explicit comparison that ignores source spans and bodies
while retaining everything that can change name resolution. The comparison
must cover:

- import path values, excluding their spans;
- top-level and nested `SymbolId`s;
- names and fully qualified names;
- `SymbolKind` and its resolver-relevant metadata.

This should be represented by a typed `ResolutionSurface` or a dedicated
comparison method in `tolk-resolver`. It must not serialize declarations to
strings for comparison.

### 4. Declaration-Level Semantic Cache

Body inference is already stored as
`WorkspaceBodyTypes` backed by `FxHashMap`, but current invalidation
replaces the complete inner map. Replace this with generation-aware entries:

```rust
struct DeclarationSemanticEntry {
    declaration: StableDeclarationKey,
    syntax_revision: DeclarationRevision,
    inference: Arc<InferenceResult>,
}
```

For body-only edits, `SymbolId` remains stable. Tree-sitter changed ranges and
the old/new declaration spans identify which declarations need inference.

Cached inference uses source spans, so declarations that move cannot blindly
reuse an old `InferenceResult`. The implementation must choose and test one of
these representations:

1. store inference spans relative to the declaration origin and rebase them at
   lookup time; or
2. provide a typed rebasing operation for unchanged inference results.

Relative storage is preferred because it makes reuse independent of edits in
earlier declarations and avoids cloning every later result on each keystroke.

### 5. Signature-Driven Dependent Invalidation

A body change can affect dependents when an inferred return type changes.
Therefore semantic invalidation proceeds in two steps:

1. infer changed declarations and refresh changed-file signatures;
2. compare old and new externally visible `TyId`s;

If visible signatures are unchanged, dependent body inference remains valid.
If a signature changed, invalidate the conservative import-dependent closure.
The first implementation does not need declaration-level cross-file dependency
tracking to be correct.

### 6. Lazy Generation-Scoped Computation

Not every edit requires all semantic products:

- definition and most references need the resolve snapshot;
- completion, hover, type definition, and type-at-position need inference for
  the enclosing declaration;
- semantic tokens and inlay hints need inference for declarations in the
  requested file;
- field references and field rename can require inference across reachable
  files;
- code actions can require file use facts.

Feature requests may fill a memoized cache for the current generation. This is
cache mutation, not semantic mutation: the result is a pure function of the
immutable generation inputs. Concurrent requests for the same key must share
or serialize the computation.

This explicitly revises the original blanket rule that feature requests never
mutate analysis state. They may populate generation-scoped memoization, as in
an IntelliJ-style analysis cache, but cannot alter source, indexes, or semantic
meaning.

### 7. Structural Snapshot Sharing

Snapshot publication currently clones all body inference, use facts, the type
interner, and `TypeDbCache`. Replace deep snapshot copies with structural
sharing:

- `Arc` per file and per declaration inference map;
- `Arc` per file use facts;
- immutable generation-owned type data;
- copy-on-write only for invalidated cache partitions.

`TolkResolveSnapshot` and semantic snapshots should contain immutable `Arc`
graphs. Publishing a generation should be close to O(number of changed
partitions), not O(workspace semantic data).

### 8. Incremental Project Topology

The complete `ProjectIndex` build costs about 3.7 ms on `acton-contracts`.
This is not the largest bottleneck, so the first resolver iteration may retain
the full topology build while reusing resolve indexes.

After resolver and snapshot costs are removed, add an incremental topology
update if the remaining index time prevents the latency target. The API should
replace one `FileIndex`, update its imports/global symbols/dependent edges, and
preserve unrelated maps.

## Hot-Path Algorithm

For a body-only edit inside an open document:

```text
incremental parse
update FileDb and FileIndex
classify old/new semantic surfaces
build or patch project topology
reuse unaffected FileResolveIndex Arcs
resolve changed file
publish resolve generation
invalidate changed declaration semantic entry
optionally infer the changed declaration eagerly
publish structurally shared semantic cache
```

Completion immediately after the edit performs at most:

```text
lookup current generation
ensure enclosing declaration inference is cached
collect context-specific completion providers
serialize result
```

No step may scan or clone every workspace file for a body-only edit.

## Correctness Rules

- A cache entry always records the generation or revisions that justify reuse.
- `SymbolId` reuse is allowed only after resolver-surface validation.
- Old and new dependent closures are both invalidated when import edges change.
- Parser-error recovery still publishes the newest syntax tree and resolver
  result available for that generation.
- If change classification is uncertain, fall back to a wider invalidation
  scope. Never reuse based on a heuristic guess.
- A stale background result is discarded when its generation no longer
  matches workspace state.
- References and rename must materialize every required type-aware usage before
  returning; partial cached results are not acceptable.

## Instrumentation

Add counters and spans that expose invalidation quality, not only elapsed time:

- `tolk.resolve.file`;
- `tolk.resolve.reused_file`;
- `tolk.type_inference.declaration`;
- `tolk.type_inference.reused_declaration`;
- `tolk.use_facts.file` and deferred fact computations;
- invalidation class counters;
- resolver/type dependent counts;
- snapshot reused and copied partition counts;
- generation cache hits and misses per feature.

Profiles must make an accidental return to workspace-wide invalidation visible
even when a microbenchmark happens to run quickly.

## Testing Plan

Tests remain Rust integration tests under
`tests/languages/tolk/`; resolver-specific invariants belong in
`tolk-resolver` tests. Snapshot output is preferred for multi-step behavior.

### Resolver Invalidation

- body edit resolves only the changed file;
- local rename updates the changed file without resolving importers;
- global declaration rename resolves transitive importers;
- declaration insertion that shifts `SymbolId`s invalidates dependents;
- import addition/removal invalidates the union of old/new closures;
- file addition/removal and mapping changes take the conservative fallback;
- reused `FileResolveIndex` values are pointer-identical where expected.

### Semantic Invalidation

- editing one function infers only that declaration;
- unchanged declarations before and after an edit retain correct types and
  source ranges;
- inferred return type changes invalidate import dependents;
- unchanged inferred return type does not invalidate dependents;
- struct field/type alias changes refresh affected member completion;
- parser errors during partial typing do not leak previous-generation types;
- recovery after completing the expression restores all semantic features.

### Feature Correctness

After every edit shape above, verify:

- definition and type definition;
- local, global, and field references;
- completion and application of the selected item;
- hover and type-at-position;
- semantic tokens and inlay hints;
- prepare rename, rename, and code actions.

### Performance Regression Tests

Wall-clock assertions are too noisy for normal CI. Deterministic tests should
assert operation counts:

- one body edit resolves one file regardless of workspace size;
- one body edit infers one declaration;
- snapshot publication copies only changed partitions.

Keep an ignored or dedicated benchmark with:

- a synthetic 300-file workspace;
- a 30 KiB edited file with many declarations;
- the Acton Jetton template;
- a source-only copy of `acton-contracts` for local release validation.

## Performance Gates

On the current `acton-contracts` benchmark and release builds:

- body-only edit to definition: mean at most 10 ms;
- edit plus member completion: mean at most 12 ms;
- typing p95: at most 16 ms;
- body-only edit resolves exactly one file;
- body-only edit infers at most the changed declaration;
- snapshot materialization: at most 1 ms mean;
- warm definition/hover/references must not regress by more than 20%;
- native and WASM must return equivalent completion, semantic-token, and
  inlay-hint results for the same stdlib/workspace inputs.

The implementation is not complete merely because it is faster than the
baseline. It is complete when it satisfies both behavior tests and these
invalidation constraints.

## Implementation Sequence

### Phase 1: Invalidation Instrumentation

- Add per-file/per-declaration counters.
- Add a differential profile integration test.
- Preserve the current behavior as a measurable baseline.

### Phase 2: Incremental Resolver

- Add structured resolution-surface comparison to `tolk-resolver`.
- Add resolver-owned reuse APIs for previous `FileResolveIndex` values.
- Resolve only invalidated files.
- Cover body, surface, import, and workspace-shape changes.

Expected result: remove approximately 45-50 ms from the large body-edit path.

### Phase 3: Structural Snapshot Sharing

- Partition body inference and facts by file/declaration behind `Arc`.
- Stop cloning complete semantic maps during publication.
- Make generation ownership explicit.

Expected result: remove most of the measured 9 ms materialization cost.

### Phase 4: Declaration-Level Semantics

- Track changed declarations from old/new syntax trees.
- introduce relative or safely rebaseable inference spans;
- infer only changed declarations;
- invalidate dependents only after visible signature changes;
- defer use facts until required.

Expected result: make semantic work proportional to the edited declaration.

### Phase 5: Lazy Feature Caches

- Add generation-scoped `ensure_*` APIs;
- make completion/hover infer only their enclosing declaration;
- make file-wide and workspace-wide features explicitly request wider scopes;
- deduplicate concurrent cache fills.

### Phase 6: Incremental Project Topology

- Implement only if the remaining full topology build prevents the performance
  gates after earlier phases.

### Phase 7: Release Validation

- Run all Tolk core and resolver tests;
- run `just clippy`;
- rebuild release native and `tolk-only` WASM;
- rerun both real-project benchmark suites;
- record before/after phase timings and result cardinalities.

## Rejected Shortcuts

### Debounce Every Change

This reduces redundant background work but does not help completion requested
immediately after a character. It also risks stale definition and rename data.

### Defer Only Type Inference

Type inference is 7 ms of the measured 75 ms rebuild. Project-wide resolution
and snapshot cloning would remain.

### Keep the Previous Snapshot Until Idle

This makes requests fast by returning stale answers. It is not acceptable for
definition, references, rename, or completion application.

### WASM-Specific Fast Path

Native exhibits the same invalidation scaling. A separate WASM implementation
would duplicate semantics without addressing the shared bottleneck.

### String-Based Declaration Fingerprints

Text slices are easy to compare but conflate formatting/body changes with
resolver and type surfaces. Invalidation must compare typed structured data.
