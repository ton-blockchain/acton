# Acton UI Package Decomposition Plan

## Status

The initial package split is complete: `acton-shared-ui` has been removed and its transaction-domain API now lives in `@acton/transaction-ui`. Generic primitives remain in `@acton/ui`, while Test UI response models are owned by `acton-test-ui`.

The target structure is:

1. Generic presentation components live in `@acton/ui`.
2. Transaction-domain components and models live in `@acton/transaction-ui`.
3. Application-specific adapters and data-fetching wrappers remain in their applications.
4. Consumers use the `--acton-*` token system instead of the legacy shared token stylesheet, with explicitly documented application-local visual tokens where the design system has no reusable semantic equivalent.
5. `acton-shared-ui` is absent from the workspace and package dependencies.

## Why the Package Was Split

The former `acton-shared-ui` package depended on TON and feature-specific libraries such as `@ton/core`, `@ton/tasm`, `@ton/tolk-abi-to-typescript`, `buffer`, `react-d3-tree`, `react-icons`, and `shiki`.

By comparison, [`@acton/ui`](/Users/petrmakhnev/emulator-rs/crates/acton-ui/package.json) is deliberately a lighter base library. Pulling the old package into it unchanged would make every primitive consumer pay for transaction-domain dependencies and would blur the boundary between design-system components and product features. Shiki is an intentional exception: it powers the generic `HighlightedCode` presentation component and is not coupled to TON transaction models.

The existing public surfaces are documented in:

- [`acton-transaction-ui/src/index.ts`](/Users/petrmakhnev/emulator-rs/crates/acton-transaction-ui/src/index.ts)
- [`acton-ui/COMPONENTS.md`](/Users/petrmakhnev/emulator-rs/crates/acton-ui/COMPONENTS.md)

## Destination Rules

### `@acton/ui`

A component belongs in the base library when it:

- is useful outside transaction inspection;
- can receive already-prepared display data through props;
- does not fetch application data or depend on application routing;
- does not require TON transaction models;
- does not introduce heavyweight domain dependencies;
- can be demonstrated independently in the component gallery.

### `@acton/transaction-ui`

A component belongs in the transaction package when it:

- renders TON transactions, messages, actions, storage changes, or decoded bodies;
- depends on transaction-domain models or parsing utilities;
- is reusable by more than one Acton application;
- is too domain-specific for the base component library.

### Application-local code

Code remains local when it owns routing, network state, address-book lookup, application APIs, workspace state, or a workflow unique to one application.

## Phase 1: Replace Existing Duplicates

This duplicate-cleanup phase is complete except for the localnet `InlineLoader`.

| Current component | Target | Notes |
| --- | --- | --- |
| Shared `Button` | `@acton/ui/Button` | Migrated; the duplicate shared API was removed. |
| Shared `Table` | `DataTableTable`, `DataTableRow`, `DataTableCell`, and related parts | Migrated; domain-specific row composition remains outside the base library. |
| Shared `DataBlock` | `RawDataBlock` | Migrated with copy, collapse, wrapping, maximum height, loading, empty, and embedded/standalone presentation. |
| `CopyValueButton`, `CopyableValue` | `CopyInlineAction` with `InlineActions` | Migrated; no additional base `CopyButton` is currently needed. |
| Explorer `Breadcrumbs` | `@acton/ui/Breadcrumbs` | Migrated through application-owned navigation adapters and prepared labels. |
| Explorer `InlineActionButton` | `InlineButton` or `InlineActions` | Migrated according to whether actions are standalone or grouped. |
| `TraceViewModeToggle` | `PillTabs` adapter | Migrated; trace-mode state and labels remain in localnet. |
| `InlineLoader` | `Skeleton` or a new `LoadingState` | Add `LoadingState` only if `Skeleton` cannot express the actual shared use cases. |

Migration should preserve controlled and uncontrolled behavior, copy feedback, collapsed state, wrapping, and keyboard interaction rather than matching only the visual result.

## Phase 2: Add Missing Base Components

### Card

Add a composable family:

- `Card`
- `CardHeader`
- `CardTitle`
- `CardDescription`
- `CardContent`
- `CardFooter`

The component should supply structure and tokens without encoding feature-specific layout.

### Form controls

`Input` is provided by `@acton/ui`. Still add alongside concrete consumers:

- `TextArea`
- `FileInput`

`JsonUploadField` itself should stay application-local as a composition of file input, textarea, validation, and workflow state.

### Tooltip

`Popover` from `@acton/ui` is the shared primitive for hover explanations. The old tooltip was not moved because it manually managed hover state and exposed inappropriate button semantics for non-interactive triggers.

The old `variant="positioned"` behavior is a transaction-tree overlay and belongs in `@acton/transaction-ui`, not in the base popover.

### AddressChip

The generic `AddressChip` accepts prepared values, including:

- `displayValue`
- optional `label`
- `copyValue`
- optional interaction callbacks

Network selection, address-book lookup, application navigation, and `ContractData` remain outside the base component. The component has no direct `@ton/core` dependency because consumers prepare its text.

The component consolidates presentation shared by the old `ContractChip`, local address renderers, and their copy behavior without importing application context.

### OpcodeChip

`OpcodeChip` is provided by `@acton/ui` behind a small structural API and composes its copy behavior from `InlineActions` and `CopyInlineAction`. Opcode `0` is rendered as the valid value `0x0` instead of being treated as missing.

### Badge and status presentation

Add a generic `Badge` or `StatusBadge` primitive for compact semantic labels. Domain code should map its state to a visual variant.

Do not duplicate exit-code parsing or rendering: `ExitCodeChip` already owns that domain presentation in `@acton/ui`.

### TonLogo

No shared `TonLogo` is currently needed. The former one-off `AppIcon` was inlined into the Test UI sidebar. Add a base component only if the same TON mark presentation is needed by multiple consumers.

### Code presentation

The planned `CodeBlock` responsibilities are intentionally split between existing primitives:

- `CodeViewer` owns file/tree-oriented code presentation and line selection;
- `HighlightedCode` owns prepared syntax-highlighted content;
- `RawDataBlock` owns generic copy, collapse, loading, empty, and raw-code presentation.

`CodeSnippet` remains application-local because fetching `/api/file` and coordinating source-file state are Test UI concerns. Do not add another base `CodeBlock` unless a concrete use case cannot be composed from the existing primitives.

Every new base component must have a gallery example and an entry in `COMPONENTS.md`.

## Phase 3: `@acton/transaction-ui` (Complete)

The reusable transaction feature now lives in a separate domain package. Its public and internal component set includes:

- `TransactionDetails`
- `ActionsSummary`
- `TransactionTree`
- `ValueFlowTable`
- `DisasmSection`
- `ContractSourcePanel`
- `SmartTooltip` and the positioned transaction overlay

Supporting transaction-domain types and utilities moved with the feature rather than being exposed through the base UI package:

- transaction and message types;
- message-body parsing;
- disassembly helpers;
- transaction formatting;
- storage-diff helpers;
- raw BoC and scalar display utilities where shared by the feature.

Tolk-specific highlighting orchestration remains outside the base package. Shiki itself is an intentional `@acton/ui` dependency used by the generic, presentation-only `HighlightedCode` component; this exception should be revisited only if highlighting is extracted into an independently reusable tooling package.

`ParsedBodySection`, `ParsedValueView`, and `ParsedValueDiffView` remain in `@acton/ui`: they are presentation-only components over prepared display values and do not depend on TON transaction models. Transaction-specific parsing and view-model preparation remain in `@acton/transaction-ui`. Storage diffs are presented through `ParsedValueDiffView`; there is no separate `StorageDiffView` component.

The feature was moved bottom-up:

1. Domain types and pure utilities.
2. Low-level transaction renderers.
3. Parsed value, action, storage, and message sections.
4. Transaction tree and transaction details compositions.
5. Application adapters and imports in localnet, test UI, and explorer UI.

The extracted package avoids internal application aliases such as `@/index`. `ContractSourcePanel` and `TransactionDetails` remain the main areas to watch for future import cycles and hidden application dependencies.

## Components That Should Remain Application-Local

- Explorer address lookup, network state, and address-book adapters; presentation uses the base `AddressChip`.
- `JsonUploadField`: owns an application workflow rather than a primitive input.
- `CodeSnippet`: fetches from the application API and coordinates highlighting/theme state.
- Gas profile views.
- Retrace workspace and stack viewer.
- Dashboard navigation and search.
- Test sidebar and summary components.
- `TraceStepsChainView`, `TraceSidePanel`, and related trace workflow components.
- Wallet-specific `SignRequestCellPreview`.

Local components should still compose `@acton/ui` primitives and, where appropriate, `@acton/transaction-ui` features.

## Phase 4: Migrate Styles and Tokens

Remove dependencies on the legacy shared token stylesheet and map all consumers to `--acton-*` tokens.

Explorer UI and Test UI have migrated from the legacy shared tokens. The remaining consumer to reduce is:

- `acton-localnet-ui`

Token migration requires visual verification because the legacy shared values are not necessarily identical to the current design system. Avoid preserving duplicate aliases indefinitely; temporary aliases must have an explicit removal point in the migration.

Localnet intentionally retains the following application-specific visual tokens:

- `--litenode-inset-highlight`: local inset treatment used by dashboard surfaces;
- `--litenode-workspace-to`: endpoint color for the localnet workspace gradient;
- `--litenode-workspace-dot`: theme-specific workspace dot color.

These are not compatibility aliases for shared UI tokens and may remain local until matching reusable semantic tokens are introduced.

## Phase 5: Remove `acton-shared-ui` (Complete)

Completed removal steps:

1. Verified there are no imports from `@acton/shared-ui`.
2. Verified no application imports its token stylesheet.
3. Removed the package from workspace and package dependencies.
4. Removed the package itself.
5. Updated component documentation and architecture notes.

Per-slice builds, type checks, lint checks, UI checks, and regression tests remain part of the ongoing verification strategy below rather than a blocker for package removal.

## Verification Strategy

For each migration slice:

- run the affected package build and type check;
- run its lint/tests;
- verify component gallery examples;
- use Playwright for pages whose visual or interactive behavior changed;
- check light and dark themes;
- verify focus, keyboard, hover, copy, tooltip, and collapsed states;
- run React Doctor before considering React changes complete;
- review bundle impact when moving dependencies across package boundaries.

If Rust code is changed as part of a later migration, run `just clippy` before that slice is considered complete.

## Acceptance Criteria

- No source file imports `@acton/shared-ui`.
- No application imports the old shared token stylesheet.
- `acton-shared-ui` is absent from workspace and package dependencies.
- `@acton/ui` does not acquire `@ton/core`, `@ton/tasm`, ABI tooling, or `react-d3-tree` without a separate, documented justification. Shiki is the documented dependency of the generic `HighlightedCode` component.
- New base components have gallery coverage and documentation.
- Transaction-domain components and utilities have a clear package boundary.
- Localnet UI, test UI, and explorer UI build successfully.
- Transaction UI tests and builds pass.
- Changed pages have been checked with Playwright in light and dark themes.
- React Doctor reports no new diagnostics.
- Bundle impact has been reviewed.
- No duplicate base primitives remain in application or transaction packages.

## Main Risks

### Visual drift

Legacy shared CSS variables may not map exactly to the current token system. Migrate and visually verify one component family at a time.

### Base-package bundle growth

TON, syntax-highlighting, and tree-rendering dependencies must not leak into `@acton/ui`. Enforce this boundary at package-manifest and import levels.

### Type coupling

Presentation components may currently accept broad shared transaction types. Replace those props with minimal view models at the package boundary.

### Hidden application context

Address formatting, address-book lookup, navigation, API fetching, and theme/highlighter setup can be embedded in apparently reusable components. Extract presentation from orchestration before moving them.

### Behavioral regressions

Copy feedback, controlled collapse state, tooltip timing, focus management, and zero-valued numeric fields can regress even when screenshots look correct. Cover these behaviors explicitly.

### Import cycles

Large components such as `ContractSourcePanel` and `TransactionDetails` may rely on internal barrels and application aliases. Move low-level dependencies first and keep package exports acyclic.

## Remaining Work

The original low-risk duplicate cleanup is complete for buttons, tables, raw-data blocks, copy actions, breadcrumbs, inline actions, and trace view-mode tabs. The remaining slices are:

1. Replace the localnet `InlineLoader` when an existing loading primitive can preserve its current full-state presentation.
2. Continue reducing the legacy Retrace token surface without replacing application-specific visualization tokens with misleading global semantics.
3. Add `Card`, `TextArea`, `FileInput`, and `Badge`/`StatusBadge` only alongside concrete consumers and gallery examples.
4. Complete light/dark Playwright verification and bundle review for the migrated applications.
