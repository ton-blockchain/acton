# Completion Implementation Plan

## Goal

Implement production-ready completion for every source language registered by
the new language server:

- TL-B;
- TASM;
- Fift;
- Tolk.

The same implementation must work through the native LSP adapter and through
the WASM/Web Worker adapter used by Monaco. Completion analysis belongs to
`ton-language-server-core`; adapters only translate protocol types.

`Acton.toml` is currently workspace configuration rather than a registered
source language. Its data can feed Tolk completion, but completion inside the
manifest itself is outside this plan.

## Reference Implementations

Use the legacy completion behavior as reference for:

- provider-based candidate collection;
- deterministic ranking and `sortText`;
- TL-B type/value context detection;
- TL-B declarations, fields, and builtin types.

Use `ton-vscode-new` as the behavioral reference for Tolk:

- insert a synthetic identifier at the caret and parse the temporary text;
- derive completion context from the resulting valid syntax tree;
- collect reference variants from lexical scope and project indexes;
- provide type-aware members after `.`;
- add context-specific keywords, snippets, annotations, imports, struct fields,
  match arms, entry points, and Acton-specific string values.

The temporary document is request-local. It must never replace the open
document, mutate the workspace snapshot, or trigger a project rebuild.

## Core Model

Add platform-neutral completion types to core:

- `CompletionList` with `is_incomplete` and ordered items;
- `CompletionItem` with label, kind, detail, documentation, filter/sort text,
  insertion format, primary text edit, and optional additional text edits;
- `CompletionTriggerKind` and request trigger character;
- `CompletionRequest` passed through `LanguagePlugin`.

Keep LSP numeric enums and `lsp-types` out of core. Native and WASM adapters
map the core model to LSP/Monaco-compatible JSON.

Use one shared collector for all languages. It owns:

- deterministic ranking;
- stable tie-breaking by label and insertion text;
- deduplication of candidates produced by overlapping providers;
- generation of `sortText`.

Completion sources are composed through the platform-neutral
`CompletionProvider<Context>` trait. A provider has only two responsibilities:

- declare whether it applies to the language-specific request context;
- add its candidates to the shared collector.

The shared runner evaluates providers in their declared order and owns the
collector lifecycle. Each language's `completion.rs` must therefore read as a
short pipeline: build one context, instantiate the providers, and invoke the
runner. Candidate discovery, syntax checks, and static completion tables live
in provider modules rather than in the orchestration function.

Provider ownership rules are strict:

- one provider type per file under the language's `completion/providers/` directory;
- the provider file owns its completion scenario and may use private helpers in
  the same file, but it must not delegate the entire scenario to a parallel
  `collect_*` module;
- providers do not import implementation details from other providers;
- shared semantic modules expose only analysis queries, such as visible-symbol
  traversal or expression-type lookup, and never accept a completion collector;
- shared item modules only convert semantic entities into ranked completion
  items and do not decide when a provider applies.

For Tolk, `providers/references.rs` owns reference, member-access, and struct
initializer completion. `semantics.rs` is the read-only adapter over the
resolver/type-inference snapshot, while `items.rs` contains reusable symbol to
completion-item conversion. This separation allows match-arm and enum providers
to reuse semantic data without depending on `ReferenceCompletionProvider`.

Providers share a language-specific context instead of taking long independent
argument lists. For workspace languages such as Tolk, that context contains
the immutable analysis snapshot, file id, request-local syntax context, open
document, and workspace completion data. Provider contexts must only borrow
this data; providers must not mutate or rebuild analysis state.

Do not create a universal completion context shared by all parsers. Each
language owns its syntax context and providers. Shared code is limited to
candidate representation, ranking, identifier-prefix/range helpers, and
provider execution and collection.

## Language Plugin Contract

Extend `LanguagePlugin` with an optional `completion` operation. The core
service performs the same lifecycle as existing features:

1. validate that the document is open;
2. check the language capability;
3. call the owning plugin with its cached parsed document;
4. profile and log the request;
5. return an empty complete list for unsupported contexts.

Completion must not be implemented as a central language switch. Adding a new
language remains: register the capability, add a language-local completion
module, and implement the plugin method.

## Language Feature Matrix

### TL-B

- Insert a synthetic identifier at the caret to recover type/value context.
- In type context, offer declarations and builtin TL-B types.
- In value context, offer fields visible in the current declaration.
- Preserve established labels, kinds, details, and ranking where they remain correct.

### TASM

- Offer all instructions from the caller-provided TVM specification.
- Include operand/signature details and short documentation.
- Insert an instruction snippet with operand placeholders when operands exist.
- Completion capability is available only when a specification is loaded,
  matching hover and code-lens behavior.

### Fift

- Offer declarations and definitions from the current file.
- Offer grammar-level words and control constructs useful in instruction and
  top-level positions.
- Keep the first version file-local, matching the current Fift resolver model.

### Tolk

Reference completion:

- visible local variables, parameters, catch variables, and type parameters;
- visible declarations from the current file, direct imports, and stdlib;
- functions, get methods, constants, globals, structs, enums, and aliases;
- fields, enum members, instance methods, and static methods after `.`;
- struct initializer fields, excluding fields already initialized;
- correct type/value filtering and declaration-name suppression.

Context completion:

- expression and statement keywords;
- control-flow and expression snippets;
- `return`, `throw`, and `assert` variants;
- top-level declarations and contract entry points;
- annotations valid for the following declaration;
- variable-size integer/bits/bytes types;
- struct field modifiers;
- enum member declarations;
- serialization method names;
- match arms for enums and union types;
- import paths for relative files, `@stdlib`, and configured mappings;
- Acton wallet names, contract ids, and get-method names where the manifest and
  indexed workspace provide them.

Insertion behavior:

- replace the identifier prefix at the caret instead of appending to it;
- use snippets for calls and constructs;
- do not add duplicate parentheses or semicolons when already present;
- retain raw/backticked source names where required;
- use additional text edits only when an import is known to be required.

## Tolk Synthetic Identifier Strategy

For every Tolk request:

1. Convert the UTF-16 caret position to a byte offset.
2. Find the identifier prefix and replacement range in the original text.
3. Insert a reserved synthetic identifier at the caret.
4. Parse the temporary source with `tolk-syntax`.
5. Locate the synthetic identifier and classify its parent/ancestors.
6. Use unchanged spans before the insertion point to query the immutable
   workspace resolve and type-inference snapshot.
7. Collect candidates without publishing or caching the temporary parse.

This keeps malformed intermediate editor text from leaking parser recovery
details into every provider. If a request cannot be classified safely, return
an empty list instead of mutating or rebuilding analysis state.

## Performance And Profiling

- Add `completion` and language-specific completion spans to the profiler.
- Tolk completion must not call workspace snapshot rebuild, project indexing,
  or whole-file type inference.
- Reuse the current immutable project index and inference results.
- Parse only the request-local temporary source.
- Keep candidate ordering deterministic so repeated requests are cache-friendly
  in clients and snapshots.

The performance snapshot must make it possible to verify that completion adds
request spans but does not increment `tolk.snapshot.rebuild` or
`tolk.type_inference.file`.

## Native And Browser Adapters

Native:

- advertise `completionProvider` with trigger characters used by the language
  set (`.`, `"`, `'`, `/`, and `@`);
- map LSP completion context into core trigger data;
- map core items, snippets, text edits, documentation, and kinds back to LSP.

WASM/Web Worker:

- export a `completion` method from `TonLanguageServer`;
- serialize the same LSP-compatible completion list as native;
- register `textDocument/completion` in the worker;
- advertise matching completion capabilities so Monaco's language client uses
  the standard LSP integration without a custom Monaco completion provider.

## Testing

All language integration tests live under
`tests/languages/<language>/completion.rs` and use file snapshots.

Core/common tests:

- ranking and deterministic `sortText`;
- deduplication precedence;
- UTF-16 replacement ranges;
- empty/unsupported completion behavior;
- completion profiling without workspace rebuilds.

TL-B snapshots:

- type position with declarations and builtins;
- value position with fields only;
- partially typed identifiers.

TASM snapshots:

- instruction list, details, and operand snippets;
- partial instruction replacement;
- no-spec behavior.

Fift snapshots:

- top-level words;
- procedure body words;
- user declarations/definitions.

Tolk snapshots:

- locals and shadowing;
- globals/imports/stdlib;
- type-only contexts;
- fields and methods after dot;
- struct initializers and match arms;
- keywords/snippets/returns/annotations;
- import paths and Acton manifest values;
- malformed source repaired by the synthetic identifier;
- no project rebuild or full inference during completion.

Adapter tests:

- native capability and LSP serialization tests;
- WASM serialization tests;
- Monaco browser test that opens each language, invokes completion, accepts an
  item, and verifies the resulting model text;
- Tolk browser test for completion after `storage.` to cover the temporary
  identifier and type-aware member path end to end.

## Implementation Order

1. Add core completion types, collector, plugin request, service dispatch,
   logging, and profiling.
2. Implement TL-B from the legacy behavior.
3. Implement TASM and Fift providers.
4. Implement Tolk synthetic context and reference variants.
5. Add Tolk context providers and insertion behavior.
6. Wire native and WASM adapters.
7. Add snapshots and browser coverage.
8. Run focused tests, all language-server tests, WASM build/tests, and
   `just clippy`; then simplify duplicated logic without changing behavior.
