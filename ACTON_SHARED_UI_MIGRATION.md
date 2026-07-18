# Acton UI Package Decomposition Plan

## Status

The initial package split is complete: `acton-shared-ui` has been removed and its transaction-domain API now lives in `@acton/transaction-ui`. Generic primitives remain in `@acton/ui`, while Test UI response models are owned by `acton-test-ui`.

The target structure is:

1. Generic presentation components live in `@acton/ui`.
2. Transaction-domain components and models live in `@acton/transaction-ui`.
3. Application-specific adapters and data-fetching wrappers remain in their applications.
4. Consumers use the `--acton-*` token system instead of the legacy shared token stylesheet.
5. `acton-shared-ui` is absent from the workspace and package dependencies.

## Why the Package Was Split

The former `acton-shared-ui` package depended on TON and feature-specific libraries such as `@ton/core`, `@ton/tasm`, `@ton/tolk-abi-to-typescript`, `buffer`, `react-d3-tree`, `react-icons`, and `shiki`.

By comparison, [`@acton/ui`](/Users/petrmakhnev/emulator-rs/crates/acton-ui/package.json) is deliberately a lighter base library. Pulling the old package into it unchanged would make every primitive consumer pay for transaction-domain dependencies and would blur the boundary between design-system components and product features.

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

These migrations should happen first because the target components already exist in `@acton/ui`.

| Current component | Target | Notes |
| --- | --- | --- |
| Shared `Button` | `@acton/ui/Button` | Migrate consumers and remove the duplicate API. |
| Shared `Table` | `DataTableTable`, `DataTableRow`, `DataTableCell`, and related parts | Preserve only domain-specific row composition outside the base library. |
| Shared `DataBlock` | `RawDataBlock` | The existing base component already covers copy, collapse, wrapping, maximum height, children, and embedded/standalone presentation. |
| `CopyValueButton`, `CopyableValue` | `CopyInlineAction` with `InlineActions` | Add a small ready-made base `CopyButton` only if repeated icon/label setup remains widespread after migration. |
| Explorer `Breadcrumbs` | `@acton/ui/Breadcrumbs` | Pass a router-aware `link` renderer and already-formatted labels from the application. |
| Explorer `InlineActionButton` | `InlineButton` or `InlineActions` | Choose according to whether the action is standalone or grouped. |
| `TraceViewModeToggle` | `PillTabs` adapter | Keep trace-mode state and labels in the application. |
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

Add:

- `Input`
- `TextArea`
- `FileInput`

`JsonUploadField` itself should stay application-local as a composition of file input, textarea, validation, and workflow state.

### Tooltip

Use the existing `Popover` from `@acton/ui` for hover explanations. The old tooltip manually managed hover state and exposed inappropriate button semantics for non-interactive triggers, so it should not be moved unchanged.

The old `variant="positioned"` behavior is a transaction-tree overlay and belongs in `@acton/transaction-ui`, not in the base popover.

### AddressChip

Create a generic `AddressChip` that accepts prepared values, for example:

- `displayValue`
- optional `label`
- `copyValue`
- optional interaction callbacks

Network selection, address-book lookup, application navigation, and `ContractData` must remain outside the base component. Avoid a direct `@ton/core` dependency if the consumer can prepare the text before rendering.

This component should consolidate the presentation shared by the old `ContractChip`, local `AddressChip`, and their copy behavior without importing their application context.

### OpcodeChip

`OpcodeChip` is provided by `@acton/ui` behind a small structural API and composes its copy behavior from `InlineActions` and `CopyInlineAction`. Opcode `0` is rendered as the valid value `0x0` instead of being treated as missing.

### Badge and status presentation

Add a generic `Badge` or `StatusBadge` primitive for compact semantic labels. Domain code should map its state to a visual variant.

Do not duplicate exit-code parsing or rendering: `ExitCodeChip` already owns that domain presentation in `@acton/ui`.

### TonLogo

Move and rename `AppIcon` to `TonLogo`. The SVG should use `currentColor` or design tokens instead of receiving a manual theme prop.

### CodeBlock

Add a presentation-only code block that accepts prepared content and state:

- code or highlighted markup;
- optional line numbers;
- optional highlighted line;
- loading and error presentation.

Do not move `CodeSnippet` wholesale. Fetching `/api/file`, following the current theme, and invoking Tolk/Shiki highlighting belong in an adapter or feature layer.

Every new base component must have a gallery example and an entry in `COMPONENTS.md`.

## Phase 3: Create `@acton/transaction-ui`

Move the reusable transaction feature into a separate domain package. Candidate components include:

- `TransactionDetails`
- `ActionsSummary`
- `TransactionTree`
- `StorageDiffView`
- `ValueFlowTable`
- `ParsedBodySection`
- `ParsedValueView`
- `DisasmSection`
- `ContractSourcePanel`
- `SmartTooltip` and the positioned transaction overlay

Move the supporting transaction-domain types and utilities with the feature rather than exposing them through the base UI package:

- transaction and message types;
- message-body parsing;
- disassembly helpers;
- transaction formatting;
- storage-diff helpers;
- raw BoC and scalar display utilities where shared by the feature.

Tolk/Shiki highlighting may deserve its own tooling package if it is reused independently. It should not become a transitive dependency of `@acton/ui`.

Move the feature bottom-up:

1. Domain types and pure utilities.
2. Low-level transaction renderers.
3. Parsed value, action, storage, and message sections.
4. Transaction tree and transaction details compositions.
5. Application adapters and imports in localnet, test UI, and explorer UI.

Avoid internal application aliases such as `@/index` in the extracted package. `ContractSourcePanel` and `TransactionDetails` need particular attention for import cycles and hidden application dependencies.

## Components That Should Remain Application-Local

- `AddressLabel`: depends on explorer hooks, network state, and address-book behavior.
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

Known consumers to check include:

- `acton-explorer-ui`
- `acton-localnet-ui`
- `acton-test-ui`

Token migration requires visual verification because the legacy shared values are not necessarily identical to the current design system. Avoid preserving duplicate aliases indefinitely; temporary aliases must have an explicit removal point in the migration.

## Phase 5: Remove `acton-shared-ui`

After all consumers have migrated:

1. Verify there are no imports from `@acton/shared-ui`.
2. Verify no application imports its token stylesheet.
3. Remove the package from workspace and package dependencies.
4. Remove the package itself.
5. Update component documentation and architecture notes.
6. Run the relevant builds, type checks, lint checks, UI checks, and regression tests.

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
- `@acton/ui` does not acquire `@ton/core`, `@ton/tasm`, ABI tooling, Shiki, or `react-d3-tree` without a separate, documented justification.
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

## Recommended First Slice

Start with the low-risk duplicate cleanup:

1. Replace shared `Button`, `Table`, and `DataBlock` consumers.
2. Consolidate copy actions.
3. Replace local breadcrumbs, inline actions, tabs, and loaders with existing base components.
4. Verify affected applications.

This reduces the old package surface before introducing new APIs and makes the remaining transaction-domain boundary easier to see.
