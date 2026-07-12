# Language Server Production Readiness Plan

## Purpose

This document describes the remaining work required to ship the Acton Language Server and the
VS Code extension as stable production components.

The current implementation already provides a broad IDE feature set, especially for Tolk. The
remaining critical work is primarily related to security, protocol compatibility, lifecycle
correctness, release engineering, responsiveness, and end-to-end validation.

## Scope

This plan covers:

- `ton-language-server-core`;
- the native `acton ls` LSP transport;
- the VS Code extension;
- compatibility with other standard LSP clients where practical;
- release and production validation.

The following work is explicitly out of scope for this plan:

- FunC language support;
- multi-root workspaces;
- replacing the existing diagnostics integration;
- complete feature parity between every supported language.

Single-root behavior must remain correct. The root selected from `InitializeParams` is authoritative,
with the CLI project root used only as a fallback.

## Current Baseline

The native server currently supports incremental document synchronization and the following standard
LSP features:

- hover;
- completion;
- signature help;
- go to definition and go to type definition;
- references;
- document highlights;
- prepare rename and rename;
- document and workspace symbols;
- semantic tokens;
- inlay hints;
- code lenses;
- folding ranges;
- quick-fix code actions;
- document and range formatting;
- file rename edits;
- watched-file notifications;
- custom profiling, disassembly, formatting, and type-at-position requests.

The VS Code extension starts `acton ls`, supports a configurable Acton executable and additional
arguments, handles restart-required settings, displays a live profiling document, and has an
Extension Host smoke test for startup, definition, incremental editing, close/reopen, and restart.

## Release Priorities

### P0: Required Before a Stable Release

#### 1. Workspace Trust and Executable Safety

The extension executes the command configured by `ton.acton.path`. Workspace settings must not be
allowed to silently select an executable in an untrusted workspace.

- [ ] Declare VS Code Workspace Trust support in `package.json`.
- [ ] Mark executable paths, language-server arguments, stdlib paths, and other command-affecting
      settings as restricted configurations.
- [ ] Do not start Acton processes while the workspace is untrusted.
- [ ] Disable trusted command links and Acton actions in untrusted workspaces.
- [ ] Resume normal activation after the user grants trust.
- [ ] Add tests for untrusted startup and trust being granted after activation.

Acceptance criteria:

- opening an untrusted repository cannot execute a binary selected by repository settings;
- granting trust starts the language server without requiring an extension reload;
- user-level trusted configuration continues to work as expected.

#### 2. Extension and Server Compatibility Contract

Acton is installed separately from the extension, so the extension must handle old and incompatible
Acton versions deliberately.

- [ ] Return the real Acton/LS version in `InitializeResult.serverInfo.version`.
- [ ] Define a versioned custom-protocol capability or protocol version in the initialize handshake.
- [ ] Let the extension verify the minimum supported Acton and protocol versions.
- [ ] Produce an actionable error for a missing `acton ls`, unsupported flags, or incompatible custom
      requests.
- [ ] Feature-detect optional custom requests instead of assuming that every installed Acton supports
      them.
- [ ] Document the extension-to-Acton compatibility policy.

Acceptance criteria:

- a compatible Acton starts without additional prompts;
- an old Acton produces one clear error with upgrade and configuration actions;
- unsupported optional functionality is disabled without breaking standard LSP features.

#### 3. Protocol Capability Correctness

Every advertised capability must have a meaningful implementation.

- [ ] Remove the advertised `tonls.tasm.stackEffect` execute command or implement its behavior.
- [ ] Audit all initialized capabilities against native handlers and core feature gates.
- [ ] Ensure unsupported language-specific operations return valid empty responses rather than RPC
      errors.
- [ ] Add a protocol snapshot test for the initialize response.
- [ ] Validate shutdown and exit behavior, including a client that disconnects unexpectedly.

Acceptance criteria:

- no advertised command is a no-op;
- common LSP clients do not report capability or response-shape errors;
- graceful shutdown does not leave an Acton process running.

#### 4. File Watching for Generic Clients and External Roots

VS Code currently creates the expected file-system watchers. Other clients should not need custom
configuration to keep a single-root index current.

- [ ] Dynamically register `workspace/didChangeWatchedFiles` when the client supports dynamic
      registration.
- [ ] Cover Tolk, TASM, Fift, TL-B, and `Acton.toml` patterns.
- [ ] Handle files in import-mapping roots outside the workspace root.
- [ ] Correctly process create, change, delete, and atomic-save rename sequences.
- [ ] Avoid indexing the same physical file through equivalent normalized paths.
- [ ] Document the required watcher behavior for clients without dynamic registration.

Acceptance criteria:

- a closed file changed outside the editor is reindexed;
- deleting and recreating a file removes stale symbols and restores them after recreation;
- changes under an external import mapping become visible without restarting the server.

#### 5. Release Pipeline and Packaged Extension Validation

CI builds a VSIX, but the artifact is not currently published or tested as the installed package.

- [ ] Upload the VSIX as a CI artifact.
- [ ] Install the generated VSIX into a clean Extension Host test environment.
- [ ] Run the smoke test against the packaged extension, not only the development directory.
- [ ] Add Marketplace and Open VSX publication jobs with protected credentials.
- [ ] Verify extension, Acton, and protocol versions during release.
- [ ] Maintain release notes and a user-visible compatibility statement.
- [ ] Test the extension on Linux, macOS, and Windows paths before a stable release.

Acceptance criteria:

- every release commit produces an installable VSIX artifact;
- the packaged extension starts a separately installed compatible Acton;
- publishing is repeatable and does not require manual file modification.

#### 6. User-Visible Failure Handling

Language-server failures must not be visible only in a hidden output channel.

- [ ] Show a concise startup failure with actions to open logs, configure Acton, and retry.
- [ ] Expose current server state: starting, indexing, ready, restarting, stopped, or failed.
- [ ] Distinguish a missing executable, incompatible version, invalid arguments, and server crash.
- [ ] Confirm that automatic LanguageClient restart and the manual restart command cannot race.
- [ ] Keep repeated failures from producing notification storms.

Acceptance criteria:

- users can diagnose and recover from a failed server without opening developer tools;
- restart always results in either a ready server or one actionable error.

### P1: Required Before Declaring the Server Mature

#### 7. Cancellation, Concurrency, and Progress

The native adapter currently serializes access to `LanguageService`. This is simple and correct, but
a long operation can delay unrelated requests.

- [ ] Introduce immutable analysis snapshots for read-only requests.
- [ ] Allow independent read-only requests to run without holding one global mutable-service lock.
- [ ] Add cancellation checkpoints to completion, references, rename, workspace symbols, and long
      indexing operations.
- [ ] Do not publish results computed for obsolete document versions.
- [ ] Report startup and large reindex operations through LSP work-done progress.
- [ ] Define latency budgets for hot and cold operations.

Initial latency targets on a representative large Acton workspace:

- hover, definition, and document highlight: below 20 ms at p95 when warm;
- completion: below 50 ms at p95 when warm;
- incremental edit invalidation: below 50 ms at p95;
- cancellation acknowledgement: below 50 ms;
- no visible editor freeze during full workspace indexing.

#### 8. Robustness and Resource Bounds

- [ ] Stress the server with long random edit sequences over valid and temporarily invalid syntax.
- [ ] Verify out-of-order and stale document versions are handled consistently.
- [ ] Bound caches or prove that they are released when documents and source files are removed.
- [ ] Add close/reopen and create/delete loops to detect stale overlays and memory growth.
- [ ] Audit panic and poisoned-lock paths so one malformed document cannot disable the session.
- [ ] Test invalid UTF-8 file reads, inaccessible files, symlink cycles, and duplicate normalized paths.

#### 9. Native LSP Contract Tests

Core feature tests do not cover all JSON-RPC and `lsp-types` conversion behavior.

- [ ] Add native request/response snapshots for every advertised capability.
- [ ] Cover UTF-16 positions with non-ASCII identifiers and comments.
- [ ] Cover `file://` URIs with spaces, authorities, and platform-specific paths.
- [ ] Cover incremental changes containing multiple edits in one notification.
- [ ] Cover didOpen, didChange, didSave, didClose, delete, and reopen as one lifecycle scenario.
- [ ] Cover malformed requests and verify that the server remains usable afterward.
- [ ] Verify custom request payloads and errors against the public protocol documentation.

#### 10. Extension Host Smoke Coverage

Keep the suite small and focused on integration boundaries rather than duplicating core tests.

- [ ] Verify completion, hover, rename, formatting, and semantic tokens through VS Code commands.
- [ ] Change runtime settings and verify refresh without restart.
- [ ] Change launch settings, accept the restart prompt, and verify the new server process.
- [ ] Verify watched external file changes and file deletion.
- [ ] Verify missing and incompatible Acton installations.
- [ ] Verify profile enablement and live profile refresh.
- [ ] Verify server crash recovery and manual restart after a crash.

## Candidate LSP Features

These features are useful, but they should be implemented after the P0 work unless a concrete product
workflow requires them earlier.

### High Value

#### Document Links

Implement `textDocument/documentLink` for:

- Tolk import paths;
- source, dependency, script, and mapping paths in `Acton.toml`;
- generated or referenced files where a stable target URI exists.

Document links complement go to definition and remain useful in clients that do not invoke definition
on strings.

#### Tolk Call Hierarchy

Implement prepare, incoming-call, and outgoing-call requests using the existing resolver and reference
index. Results must distinguish overloads and methods with the same source name.

#### Organize Imports

Add a `source.organizeImports` code action that:

- removes unused imports only when semantic certainty is available;
- sorts imports using formatter/project conventions;
- preserves configured import groups;
- never rewrites unresolved imports destructively.

#### Semantic Tokens Delta and Range

Support range requests and full deltas to avoid resending all semantic tokens after small edits. Add
result IDs tied to document versions and fall back to a full response when the previous result is no
longer available.

#### Selection Ranges

Implement syntax-tree-backed `textDocument/selectionRange` for predictable expand-selection behavior
in all tree-sitter languages.

### Product-Specific Value

#### Inline Values for Debugging

Implement `textDocument/inlineValue` if it can share the debugger's source mapping and variable
model. Keep runtime values in the debug integration rather than the static type engine.

#### Completion Item Resolve

Use `completionItem/resolve` only when profiling demonstrates value. It can defer expensive
documentation or metadata, but should not make basic completion insertion dependent on another
round trip.

#### Standard Diagnostics

The existing diagnostics integration remains unchanged for this plan. A future migration to LSP pull
diagnostics would make diagnostics available consistently to browser and non-VS Code clients.

When implemented, diagnostics must be versioned, cancellable, and shared with the existing compiler
and linter logic rather than reimplemented in the language server.

### Low Priority

Do not prioritize these without a concrete workflow:

- go to implementation;
- type hierarchy;
- linked editing ranges;
- document colors;
- monikers;
- notebook synchronization.

## Language Feature Follow-Up

Feature parity is not required for the first stable release, but the extension must publish an honest
feature matrix.

### Tolk

Tolk already has the broadest feature surface. Prioritize correctness, performance, call hierarchy,
document links, and organize imports over adding more isolated providers.

### Fift

Current support includes completion, folding, definition, references, hover, code lenses, inlay hints,
and semantic tokens.

Potential follow-up:

- document symbols;
- document highlights;
- rename where symbol identity is unambiguous;
- signature help for callable words where the specification provides enough information.

### TL-B

Current support includes completion, definition, references, hover, document symbols, inlay hints,
and semantic tokens.

Potential follow-up:

- folding ranges;
- document highlights;
- single-file rename;
- selection ranges.

### TASM

Current support includes completion, hover, folding, and stack-effect code lenses.

Potential follow-up:

- document symbols for named blocks and declarations;
- definition and references for local symbolic targets;
- semantic tokens where TextMate highlighting lacks semantic information;
- a non-clickable presentation for informational stack-effect code lenses, unless the associated
  command gains a real action.

### Acton.toml

Current core support includes completion and hover, with additional extension-side code lenses and
hover actions.

Potential follow-up:

- document links and definitions for paths and contract references;
- document symbols;
- formatting;
- schema diagnostics when diagnostics are moved into the LS;
- moving generic TOML semantics from the VS Code client into core when browser and other LSP clients
  need the same behavior.

## Documentation

- [ ] Keep the custom request documentation synchronized with native and WASM implementations.
- [ ] Publish the supported language/feature matrix.
- [ ] Document Acton discovery order, custom executable configuration, and minimum version.
- [ ] Document single-root behavior and the absence of multi-root support.
- [ ] Document profiling startup, profile retrieval, and interpretation of major spans.
- [ ] Document troubleshooting for startup, incompatible versions, missing stdlib, and stale indexes.

## Proposed Delivery Sequence

### Milestone 1: Safe Beta

- Workspace Trust and restricted settings.
- Server/protocol version handshake.
- Honest execute-command and initialize capabilities.
- Actionable startup failures.
- VSIX artifact upload and packaged-extension smoke test.

### Milestone 2: Reliable Single-Root Release

- Generic-client watched-file registration.
- External mapping-root lifecycle.
- Complete native LSP lifecycle snapshots.
- Extension configuration, crash, and restart smoke scenarios.
- Cross-platform path validation.

### Milestone 3: Responsive Stable Release

- Cancellation and work-done progress.
- Concurrent read-only analysis over immutable snapshots.
- Stress, memory, and stale-version tests.
- Published latency measurements on representative small and large projects.

### Milestone 4: IDE Depth

- Document links.
- Tolk call hierarchy.
- Organize imports.
- Semantic token deltas and ranges.
- Selection ranges.
- Selected language-specific follow-up from the published feature matrix.

## Stable Release Exit Criteria

The language server and VS Code extension are ready for a stable production release when:

- untrusted workspaces cannot execute repository-selected commands;
- a compatible external Acton is discovered and verified reliably;
- incompatible Acton versions fail with an actionable message;
- every advertised LSP capability and command has tested behavior;
- closed and externally changed files cannot leave stale symbols in a single-root workspace;
- cancellation and progress keep the editor responsive on a large project;
- the native protocol suite covers lifecycle, UTF-16 conversion, and custom requests;
- a clean VS Code instance can install the CI-produced VSIX and pass the smoke suite;
- CI publishes reproducible artifacts for supported platforms;
- the language feature matrix and custom protocol documentation match the shipped implementation;
- no known P0 issue remains open.
