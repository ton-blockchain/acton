# Acton UI Component Catalog

This catalog is a text companion to `acton-ui-gallery`. It is written for both
humans and coding agents that need to build future Acton UI without guessing
which primitives exist or when to use each visual variant.

This is not prop documentation. Prefer the TypeScript source for exact API
details.

## Button

Status: ready

Import:

```tsx
import { Button } from "@acton/ui"
```

Use Button for explicit user-triggered commands: submit, confirm, start, stop,
create, delete, or any action that changes application state. Do not use it for
route navigation or passive status display.

### Variants

- `primary`: the main action in a focused flow. It uses inverted neutral colors:
  dark text color on light theme becomes a dark button, while dark theme becomes
  a light button. Use at most one primary button in a local action group.
- `secondary`: the default neutral action when no choice should dominate.
- `outline`: a low-emphasis action that still needs a visible boundary.
- `ghost`: low-emphasis toolbar actions and compact repeated controls.
- `danger`: destructive actions with clear intent or confirmation nearby.

### Sizes

- `sm`: dense toolbars, table rows, and compact control groups.
- `md`: default forms, panels, and application controls.
- `lg`: primary actions in sparse layouts.
- `icon`: icon-only toolbar actions. Always provide an accessible label.

### States To Review Visually

- Default
- Disabled
- Loading
- Danger disabled
- Text only
- Leading icon
- Trailing icon
- Icon only

### Agent Guidance

- Prefer `Button` for command actions.
- Keep labels short and action-oriented.
- Avoid mixing several high-emphasis variants in the same compact group.
- Reserve `danger` for destructive operations; do not use it for reversible or
  harmless actions.
- Use `ghost` for repeated low-emphasis controls where solid buttons would add
  visual noise.
- Use `InlineButton` instead of `Button` plus custom classes for Debug-style
  actions embedded inside existing content.

## Breadcrumbs

Status: ready

Import:

```tsx
import { Breadcrumbs } from "@acton/ui"
```

Use Breadcrumbs for compact page hierarchy and explorer paths such as account,
block, ABI catalog, source catalog, and transaction trace pages. It renders a
semantic breadcrumb nav while keeping links router-agnostic.

### Composition

```tsx
<Breadcrumbs
  items={[
    { label: "Explore", link: (children, className) => <Link to="/" className={className}>{children}</Link> },
    { label: address, link: (children, className) => <Link to={addressPath} className={className}>{children}</Link> },
    { loading: true, loadingLabel: "Loading transaction hash", skeletonWidth: "18rem" },
  ]}
/>
```

- `items`: ordered path segments.
- `label`: visible segment content.
- `link`: optional callback that receives children and className, so callers can
  use `react-router`, anchors, or another router without coupling `@acton/ui`.
- `current`: marks the page segment. If omitted, the last item without `link`
  is treated as current.
- `loading`: renders a Skeleton segment in place of the label.
- `skeletonWidth`: controls the width of a loading segment.
- `loadingLabel`: accessible label for a loading segment.
- `truncate`: set to `false` for short stable segments, or `"middle"` for
  string/number technical values where the suffix must remain visible.
- `preserveEnd`: number of trailing characters to keep when
  `truncate="middle"`.
- `preserveStart`: number of leading characters to keep when
  `truncate="middle"`.

### States To Review Visually

- Basic path
- Current final segment
- Long address/hash truncation
- Partial skeleton segment
- Multiple loading segments
- Mobile wrapping

### Agent Guidance

- Use Breadcrumbs for page hierarchy and technical path context.
- Keep address/hash formatting outside Breadcrumbs; pass already formatted
  labels or custom React nodes.
- Set `truncate: false` on stable short labels that must never collapse.
- Set `truncate: "middle"` on addresses, hashes, and long technical ids when
  the suffix is useful.
- Use item-level `loading` when only part of the path is unresolved.
- Do not replace the whole breadcrumb row with a custom skeleton if stable path
  segments are known.
- Use `link` for router integration instead of adding router dependencies to
  `@acton/ui`.
- Use Breadcrumbs only for ordered ancestors of the current page.

## InlineButton

Status: ready

Import:

```tsx
import { CopyInlineButton, InlineButton } from "@acton/ui"
```

Use InlineButton for embedded command actions inside rows, cards, details
panels, and compact metadata groups. It keeps real button semantics but does not
draw a boxed control surface.

### Variants

- `default`: neutral embedded action inside dense UI.
- `utility`: micro-sized copy, reveal, and raw-data utility action with a small
  leading icon and text label.
- `accent`: debug, inspect, reveal, or related tool action that should read like
  the Debug button in localnet-style surfaces.
- `danger`: destructive embedded action with clear local context.

### States To Review Visually

- Default
- Utility
- Accent
- Danger
- Embedded row context
- Copy feedback (`CopyInlineButton`)

### Copy Actions

Use `CopyInlineButton` for a text-and-icon copy command. It composes
`InlineButton` with the `utility` variant, copies `value`, switches to a check
icon and `Copied` text, and resets after 2000ms by default.

```tsx
<CopyInlineButton
  value={rawMessage}
  label="Copy raw message"
  copiedLabel="Copied raw message"
>
  Copy raw message
</CopyInlineButton>
```

### Agent Guidance

- Prefer `InlineButton` for Debug-style actions inside existing content.
- Use `utility` for compact text+icon commands such as `Copy raw body` or
  `Copy raw state init`.
- Use `CopyInlineButton` instead of wiring clipboard and copied state by hand.
- Keep `utility` visually smaller than the default/accent variants; it should
  feel like an inline caption action, not a regular button.
- Pair `accent` with a small lucide icon when the action benefits from quick
  recognition.
- Keep labels short; these actions live inside already dense UI.
- Do not add background, border, or fixed control height through `className`.
- Do not use InlineButton for primary form actions, standalone footer actions,
  or navigation to another route.

## InlineActions

Status: ready

Import:

```tsx
import { CopyInlineAction, InlineAction, InlineActions } from "@acton/ui"
```

Use InlineActions when an inline value needs one or more compact icon-only
actions, for example copying an address, opening a linked entity, or removing a
row item.

### Composition

```tsx
<InlineActions
  visibility="hover"
  actions={
    <CopyInlineAction
      value={address}
      label="Copy address"
      copiedLabel="Address copied"
      icon={<Copy />}
      copiedIcon={<Check />}
    />
  }
>
  <AddressLabel address={address} />
</InlineActions>
```

- `children`: the inline content that owns the hover/focus area.
- `actions`: one or more `InlineAction` buttons rendered to the right.
- `visibility="hover"`: actions are hidden until hover or focus-within, and are
  always visible on coarse pointer devices.
- `visibility="always"`: actions are always visible.
- `InlineAction` uses a 20px control with a 13px icon to match existing inline
  copy actions in localnet/shared UI.
- `size="compact"` uses a 16px control with an 11px icon for tight chips and
  metadata values.
- `CopyInlineAction` copies `value`, switches to `copiedIcon`, updates the
  accessible label/title, and resets after 2000ms by default.
- `CopyInlineAction` uses copy/check icons by default; pass `icon` and
  `copiedIcon` only when the surrounding domain needs different symbols.

### Visibility Rules

- Use `hover` for low-risk helper actions such as copy hash, copy address, copy
  id, or reveal raw value.
- Use `always` for remove actions, important row actions, and contexts
  where discoverability matters more than visual quiet.
- Hover reveal must also work on keyboard focus; do not implement a hover-only
  custom wrapper outside this component.

### Agent Guidance

- Keep the inline content inside `InlineActions` children.
- Use `InlineAction` for icon-only buttons and always provide a clear `label`.
- Use `CopyInlineAction` instead of wiring copy state by hand when the action
  should change to a check mark after copying.
- Keep `InlineAction` visually neutral; handle destructive intent through label,
  placement, confirmation, or surrounding context.
- Use `size="compact"` only when the surrounding value is itself compact; keep
  the default size for standalone inline actions.
- Use `InlineButton` instead when the action needs a visible text label like
  `Debug`.
- Do not hide remove actions behind hover-only visibility.

## Input

Status: ready

Import:

```tsx
import { Input } from "@acton/ui"
```

Use Input for standalone single-line form values, filters, editable names,
amounts, endpoints, addresses, and code hashes. It keeps native input behavior
and forwards native attributes and refs.

### Composition

```tsx
<Input
  label="Code hash"
  description="Hex or base64 contract code hash."
  mono
  spellCheck={false}
/>
```

- `size`: `sm`, `md`, or `lg`; `md` is the default.
- `label`: optional accessible field label. Omit it when the caller already
  renders a linked label.
- `description`: optional neutral helper text linked with
  `aria-describedby`.
- `invalid`: applies validation styling and `aria-invalid` without owning an
  error message.
- `leadingIcon`: optional decorative icon for standard search, token, or
  credential fields. The icon is hidden from assistive technology.
- `shortcut`: optional global modifier shortcut key, such as `"K"`. Input
  renders the platform modifier and focuses and selects itself when the
  shortcut is pressed.
- `mono`: uses the shared monospace stack for hashes, addresses, and other
  technical values.
- `fieldClassName`: layout hook for the optional label/description wrapper;
  `className` always targets the native input.

### States To Review Visually

- Empty and populated
- Hover and keyboard focus
- Read-only and disabled
- Invalid
- Required label
- Small, medium, and large
- Search, password, number, and monospace values

### Agent Guidance

- Use Input for a single-line native input value.
- Prefer the built-in label and description when no surrounding form field
  owns them.
- Use `invalid` or `aria-invalid` to expose validation state, then report the
  actual validation or request failure through Toast.
- Input defaults `autoComplete`, `autoCorrect`, and `autoCapitalize` to `off`,
  and `spellCheck` to `false`, because most Acton values are technical. Override
  them explicitly for credential, email, name, or prose-like fields.
- Keep file inputs native and use Checkbox for boolean values.
- Do not add leading icons, shortcuts, unit suffixes, or embedded buttons
  through absolute positioning outside Input. Use `leadingIcon` for decorative
  icons and `shortcut` for a focus shortcut; unit suffixes and interactive
  actions belong in a dedicated InputGroup.
- Do not add an inline error-message API to Input.

## SearchInput

Status: ready

Import:

```tsx
import { SearchInput } from "@acton/ui"
```

Use SearchInput for a search field that reveals recent queries or resolved
matches. It owns the field, floating list, focus transitions, removable rows,
and one-line or two-line item layout. The caller continues to own lookup,
history persistence, validation messages, and navigation.

### Composition

```tsx
<SearchInput
  ariaLabel="Explorer search"
  value={query}
  items={matches.map(match => ({
    id: match.address,
    label: match.name,
    description: match.address,
    onSelect: () => openAddress(match.address),
  }))}
  onValueChange={setQuery}
  onSubmit={search}
/>
```

- `size`: `sm` for headers and dense toolbars, or `lg` for primary page search.
- `items`: resolved rows with a stable `id`, label, optional description and
  icon, and an `onSelect` callback.
- `onRemove`: optional history-row command. SearchInput supplies the remove
  button, accessible label, and focus behavior.
- `onSubmit`: handles Enter; return `false` when validation fails and the
  control should stay open.
- `open` and `onOpenChange`: optional controlled visibility for previews or
  coordinated layouts. Normal product usage can leave visibility uncontrolled.

### States To Review Visually

- Empty query with removable history
- Query with one-line matches
- Query with two-line matches
- Small and large sizes
- Invalid control
- Empty result list
- Keyboard focus moving from the input into a result or remove button

### Agent Guidance

- Keep domain lookup, `localStorage`, router calls, and toasts outside the base
  component.
- Pass already formatted labels and descriptions; SearchInput does not know
  about TON addresses or ABI declarations.
- Use `description` only for secondary identity such as an address, kind, or
  opcode.
- Let SearchInput own open/close and blur behavior instead of adding delayed
  timers in feature code.
- Use Input for a search field without a dropdown, and use a command palette
  for global application commands.

## AddressChip

Status: ready

Import:

```tsx
import { AddressChip } from "@acton/ui"
```

Use AddressChip for a compact technical address that may need copying,
navigation, caller-resolved naming, or coordinated highlighting. The component
owns shortening, copy feedback, focus behavior, and the compact chip visual.

### Composition

```tsx
<AddressChip
  address={address}
  formatAddress={formatFriendlyAddress}
  label={resolvedName ? <span>{resolvedName}</span> : undefined}
  onAddressClick={openAccount}
/>
```

- `address`: raw or already formatted string; missing values render `fallback`.
- `formatAddress`: optional caller-owned conversion to a friendly/network form.
- `label`: optional resolved display content that replaces the shortened address.
- `copyable`: hides the copy action when another surrounding action owns the row.
- `copyPlacement`: places the copy action on the left or right without changing
  click semantics.
- `shorten`: controls compact middle truncation; full values wrap safely.
- `highlighted`: coordinates the chip with another hovered address or trace node.
- `variant="plain"`: uses the standard text color, removes address hover styling,
  and keeps the copy action visible.
- `copied`: optional external copied state; the component also owns local feedback.
- `onCopyAddress`: optional application copy handler. Native clipboard copying is
  used when omitted.

### States To Review Visually

- Missing address and custom fallback
- Shortened and full address
- Caller-resolved label
- Clickable and read-only
- Copy action on the left and right
- Highlighted address
- Keyboard focus and copied feedback

### Agent Guidance

- Keep TON parsing, network selection, and address-book lookup outside AddressChip.
- Pass a formatter callback instead of adding `@ton/core` to `@acton/ui`.
- Pass resolved names through `label`; do not add address-book state to the component.
- Let AddressChip own shortening and copy feedback.
- Use ContractChip when known contract metadata should show a letter, name, and
  address together.
- Do not wrap AddressChip in another copy control or clickable element.

## ContractChip

Status: ready

Import:

```tsx
import { ContractChip } from "@acton/ui"
```

Use ContractChip for TON addresses that may have a known contract identity. It
renders minimal metadata (`letter` and `displayName`), falls back to the
address, and composes copying from `CopyInlineAction`.

### States To Review Visually

- Missing address
- Unknown contract with shortened address
- Known contract
- Caller-formatted address
- Clickable contract plus independent copy action

### Agent Guidance

- Pass a `ReadonlyMap<string, ContractChipData>`; richer domain contract records
  are structurally compatible, but ContractChip does not consume their ABI.
- Pass `formatAddress` when network-specific friendly formatting is required.
- Keep TON parsing libraries outside `@acton/ui`; the formatter callback owns
  that policy.
- Use `onContractClick` for navigation and let the component keep the copy
  action as a sibling rather than nesting interactive elements.
- Do not wrap ContractChip in another copy control.

## ParsedValueView

Status: ready

Import:

```tsx
import { ParsedValueView, type ParsedValue } from "@acton/ui"
```

Use ParsedValueView after domain code has decoded an ABI, message body, or
storage cell into the minimal exported `ParsedValue` union. It renders scalar,
address, boolean, null, void, array, object, and map nodes recursively.

### States To Review Visually

- Null and void
- True and false
- Decimal, hexadecimal, coin, and raw-copy scalars
- Known and unknown addresses
- Empty and populated arrays
- Empty and populated objects
- Empty and populated maps
- Nested structures on narrow screens

### Agent Guidance

- Keep ABI lookup, symbol tables, cells, tuple readers, and parsing outside the
  component.
- Depend only on the minimal `ParsedValue` shape; richer parser output can be
  structurally compatible.
- Pass `fieldName` for isolated scalars when field-sensitive hex or coin display
  rules should apply.
- Pass `rawValue` on a scalar to enable the shared compact copy action.
- Reuse ContractChip metadata and `formatAddress` for address nodes.
- Do not duplicate the recursive array, object, or map layout in callers.

## ParsedValueDiffView

Status: ready

Import:

```tsx
import { buildStorageDiff, ParsedValueDiffView } from "@acton/ui"
```

Use `buildStorageDiff` to compare two minimal named `ParsedValue` roots and
render the result with ParsedValueDiffView. The builder and renderer depend only
on presentation models; ABI lookup, cell decoding, and parser policy stay in
domain code.

### States To Review Visually

- Unchanged leaf
- Changed leaf
- Added and removed leaves
- Address changes with known contract metadata
- Serialized cell-like values with raw copy actions
- Nested object and array changes
- Nested map additions, removals, and changed values
- Empty containers

### Agent Guidance

- Pass `{ name, value }` as `ParsedStorageValue`; richer decoded storage records
  are structurally compatible.
- Build the diff once outside render when the before/after values are stable.
- Pass ContractChip metadata and `formatAddress` for address leaves.
- Provide a `ParsedValueDiff` directly only when comparison already belongs to
  domain code.
- Do not pass ABI objects, cells, dictionaries, or parser contexts.
- Do not duplicate ParsedValueView leaf formatting inside diff consumers.

## ParsedBodySection

Status: ready

Import:

```tsx
import { ParsedBodySection } from "@acton/ui"
```

Use ParsedBodySection for an accessible disclosure around a decoded body. It
accepts only `{ name, value }`, delegates the tree to ParsedValueView, and does
not know how the ABI was selected or parsed.

### States To Review Visually

- Collapsed
- Expanded
- Custom title
- Nested body wider than its container
- Missing body, which intentionally renders nothing

### Agent Guidance

- Pass an already decoded `ParsedTransactionBody`.
- Use `defaultExpanded` only when decoded data should be immediately visible.
- Use `title` for another body-like decoded structure such as storage.
- Do not add another disclosure control around ParsedBodySection.
- Do not perform decoding or ABI fallback inside the component.

## OpcodeChip

Status: ready

Import:

```tsx
import { OpcodeChip } from "@acton/ui"
```

Use OpcodeChip for TON message opcodes that may have a domain-resolved ABI name.
It owns hexadecimal formatting and composes its copy interaction from
`InlineActions` and `CopyInlineAction`.

### Composition

```tsx
<OpcodeChip
  opcode={opcode}
  abiName={resolvedOpcodeName}
  showOpcode
/>
```

- `opcode`: numeric opcode. Zero is valid and renders as `0x0`; `undefined`
  renders `Empty` without a copy action.
- `abiName`: optional symbolic name resolved by domain code.
- `showOpcode`: keeps the hexadecimal value visible beside `abiName`.
- The copy action appears on hover or keyboard focus and is always visible on
  coarse pointer devices.

### States To Review Visually

- Missing opcode
- Zero opcode
- Numeric opcode without an ABI name
- ABI name with hexadecimal secondary text
- ABI name without secondary text
- Copy hover/focus and copied state

### Agent Guidance

- Pass a number and let OpcodeChip format the hexadecimal value.
- Resolve ABI names outside `@acton/ui` and pass the prepared string.
- Do not wrap OpcodeChip in another copy control.
- Do not duplicate copy state, timers, clipboard calls, or copy/check icons in
  consumers.

## DisclosureToggle

Status: ready

Import:

```tsx
import { DisclosureToggle } from "@acton/ui"
```

Use DisclosureToggle for compact inline Show/Hide controls that reveal content
below or nearby: parsed body, state init, disassembled code, storage diffs, and
action details. It owns button semantics, chevron state, visible state text,
loading state, and `aria-expanded`.

### Composition

```tsx
<span className={styles.sectionLabel}>Parsed Body</span>
<DisclosureToggle
  expanded={isParsedBodyOpen}
  contextLabel="parsed body"
  onClick={() => setParsedBodyOpen(open => !open)}
/>
```

- `expanded`: controls the chevron direction and Show/Hide label.
- Keep stable section labels such as `Parsed Body`, `State Init`, and `Code`
  outside the component so hover changes only the chevron and state text.
- `loading`: disables the button, sets `aria-busy`, and switches to
  `loadingLabel`.
- `contextLabel`: used to generate an accessible label such as
  `show parsed body` or `hide actions`.

### States To Review Visually

- Closed after external muted label
- Open after external muted label
- Closed after external numeric value
- Loading after click
- Custom `Load`/`Loading` labels

### Agent Guidance

- Use DisclosureToggle instead of hand-writing chevron plus Show/Hide buttons.
- Keep expanded content outside the component; DisclosureToggle is only the
  trigger.
- Use `contextLabel` whenever the visible label is short or numeric.
- Keep labels and values outside DisclosureToggle. Do not add label/value props
  to the component; local layouts own that content.
- Do not use DisclosureToggle for full-width collapsible headers, large
  accordions, or register-form buttons.

## ExitCodeChip

Status: ready

Import:

```tsx
import {ExitCodeChip} from "@acton/ui"
```

Use ExitCodeChip for TVM compute-phase exit codes and transaction action result
codes. It owns success classification, standard TVM descriptions, contract ABI
error lookup, and the contextual popover.

### Minimal ABI Shape

The `abi` prop is structural and intentionally limited to the data the chip
reads:

```tsx
interface ExitCodeAbi {
  readonly thrown_errors?: readonly {
    readonly err_code: number
    readonly name?: string
    readonly description?: string
  }[]
}
```

A full compiler ABI is assignable to this shape, but `@acton/ui` does not import
or depend on compiler ABI types.

### States To Review Visually

- Missing code: yellow `Unknown` chip
- Compute success: 0 and 1
- Action success: 0
- Standard TVM compute error
- Standard action error
- Contract-defined ABI error
- Unknown custom error

### Agent Guidance

- Set `phase="action"` for action result codes; compute is the default.
- Pass `abi` only when contract-defined thrown errors are available.
- Pass `undefined` while the result is unavailable; the component renders an
  `Unknown` warning chip.
- Import the component directly from `@acton/ui`; do not recreate standard exit
  code descriptions in application code.
- Do not add a full ABI package dependency to consumers that only need the chip.

## ModeViewer

Status: ready

Import:

```tsx
import {
  ChangeLibraryModeViewer,
  ModeViewer,
  ReserveModeViewer,
  SendModeViewer,
} from "@acton/ui"
```

Use the three domain wrappers for TON action modes. They share the same inline
layout, separators, empty state, and explanatory popovers, while each wrapper
keeps its bit parsing in a separate parser module.

### Wrappers

- `ReserveModeViewer`: reserve base mode and optional reserve flags.
- `SendModeViewer`: independent message send flags and regular mode `0`.
- `ChangeLibraryModeViewer`: library visibility, bounce behavior, and unknown
  bits.

All wrappers accept `mode: number | undefined`. An unavailable value renders
`No mode`.

### Base Component

`ModeViewer` accepts `mode` and a `parseMode` function returning entries with
`name`, `value`, `description`, and an optional `docsUrl`. Use it directly only
when adding another mode family; callers rendering the three known TON modes
should use a wrapper.

Descriptions may be plain strings or composed parts. Use `{name, value}` parts
for inline mode constants and `{code}` parts for Tolk functions, types, and
other code references.

### Agent Guidance

- Keep every mode family's constants and bit rules in its own folder and parser
  file.
- Reuse `ModeViewer` for presentation instead of rebuilding separators and
  popovers.
- Do not add reserve, send, or library-specific branches to `ModeViewer`.
- Do not use one domain wrapper for a different mode family even when bit values
  overlap.

## ContentTabs

Status: ready

Import:

```tsx
import { ContentTabs } from "@acton/ui"
```

Use ContentTabs for compact connected tabs above a bordered panel when the user
switches between alternate representations of the same content: disasm,
base64, hex, hashes, parsed data, tables, or trace sections.

### Composition

```tsx
<ContentTabs
  ariaLabel="Contract code formats"
  tabs={tabs}
  value={activeTab}
  onValueChange={setActiveTab}
  panelClassName={styles.codePanel}
>
  <pre>{code}</pre>
</ContentTabs>
```

- `tabs`: ordered tab labels and string values.
- `value`: current selected tab.
- `onValueChange`: updates selected tab from click or keyboard navigation.
  It may return a Promise when switching tabs requires async loading.
- `children`: the complete panel content for the current tab.
- `loading`: shows loading fallback for externally managed async state.
- `loadingValue`: optional tab value to mark selected while external loading is
  active.
- `loadingFallback`: optional custom placeholder; defaults to `SkeletonText`.
- `panelClassName`: optional panel layout for code scrolling, table layout, or
  local padding.
- `listClassName`: optional tab-list layout override for narrow host surfaces.

### States To Review Visually

- Code viewer with several data formats
- Long scrollable panel content
- Arbitrary table or structured content
- Promise-based tab loading with skeleton fallback
- Horizontal tab overflow on narrow screens
- Keyboard focus and arrow navigation

### Agent Guidance

- Use ContentTabs for tabbed content panels, not one-off tab button markup.
- Keep domain-specific decoding and rendering outside the component.
- Keep the component controlled so app state decides which content is shown.
- Return a Promise from `onValueChange` when a tab switch needs async work;
  ContentTabs immediately selects the target tab and shows the loading fallback
  until the Promise settles and controlled `value` catches up.
- Use `loading`/`loadingValue` when async state is owned outside
  `onValueChange`.
- Use `Skeleton` or `SkeletonText` for custom `loadingFallback` layouts.
- Use `panelClassName` for content-specific sizing; do not bake code/table
  assumptions into ContentTabs.
- Use a future segmented control instead for small view-mode switches without a
  connected content panel.
- Do not use ContentTabs when tabs do not share the same framed content area.

## PillTabs

Status: ready

Import:

```tsx
import { PillTab, PillTabs, PillTabToggle } from "@acton/ui"
```

Use PillTabs for detached pill-like selector rows, for example Test UI trace
selection, compact item filters, and selectors with a collapsible group summary.
It does not own a content panel.

### Composition

```tsx
<PillTabs ariaLabel="Trace selector">
  <PillTabToggle expanded={showDeploys} onClick={() => setShowDeploys(open => !open)}>
    2 treasury deploys
  </PillTabToggle>
  {showDeploys && <PillTab variant="muted">Trace 1</PillTab>}
  <PillTab selected={selectedTrace === 3} onClick={() => setSelectedTrace(3)}>
    Trace 3
  </PillTab>
</PillTabs>
```

- `PillTabs`: horizontal row with stable gap and horizontal overflow.
- `PillTab`: selectable button with `selected`, `disabled`, and `variant`.
- `variant="default"`: regular selector item.
- `variant="muted"`: group child, skipped item, or lower-emphasis selector.
- `variant="group"`: summary pill, normally through `PillTabToggle`.
- `PillTabToggle`: group summary button with chevron and `aria-expanded`.

### States To Review Visually

- Group toggle open and closed
- Default selected tab
- Muted group child tab
- Disabled skipped tab
- Narrow horizontal overflow

### Agent Guidance

- Use PillTabs for trace selector rows and other detached item selectors.
- Use ContentTabs when the tabs are visually connected to a content panel.
- Keep trace filtering, selected item state, and domain labels in the caller.
- Do not use PillTabToggle as the selected content tab; it controls group
  visibility.

## VisuallyGroupedNumber

Status: ready

Import:

```tsx
import { VisuallyGroupedNumber } from "@acton/ui"
```

Use VisuallyGroupedNumber for long decimal technical values where visual
readability matters but the underlying string must not gain real separators:
parsed scalar values, storage diffs, balances, gas values, counters, and ids.

### Composition

```tsx
<VisuallyGroupedNumber className={styles.value} value={displayValue} />
```

- `value`: already formatted display value. Plain decimal strings are visually
  grouped; short values and non-decimal strings render unchanged.
- `className`: caller-owned typography, color, truncation, and layout.

### States To Review Visually

- Short values
- Large decimal values
- Signed values
- Decimal fractions
- Hex/hash strings that should remain unchanged

### Agent Guidance

- Use it only when visual grouping should not alter copyable text.
- Keep domain formatting outside the component.
- Do not use it for addresses, hashes, base64, or values that need middle
  truncation.
- Do not insert literal spaces into technical numbers for readability.

## CodeViewer

Status: ready

Import:

```tsx
import { CodeViewer } from "@acton/ui"
```

Use CodeViewer for read-only multi-file source bundles. It owns the collapsible
file tree, active-file selection, entrypoint marker, line numbers, syntax
highlighting, copy action, optional external action, and responsive file picker.

### Composition

```tsx
<CodeViewer
  files={sourceFiles}
  entrypoint="contracts/main.tolk"
  externalActionUrl={verificationUrl}
  externalActionLabel="View verification"
/>
```

- `files`: minimal `{path, content}` records; domain-specific source metadata
  stays in the caller.
- `entrypoint`: optional path selected initially and marked as `main`.
- `externalActionUrl` and `externalActionLabel`: optional navigation to a
  verifier, repository, artifact, or other source context.
- `compact`: limits the source height for dense details panels.
- `attachedToTabs`: removes the leading top corner radius when the viewer sits
  directly below a tab bar.
- `emptyMessage`: caller-provided text for an empty source bundle.

### States To Review Visually

- Nested expanded folders
- Independently collapsed folders
- Active and hovered files
- Entrypoint marker
- Compact layout
- Mobile file picker
- Empty source bundle

### Agent Guidance

- Use it for verified contracts, generated source bundles, and other read-only
  multi-file code.
- Keep loading, verification, compilation, decompilation, and source-domain
  types outside the component.
- Let CodeViewer own file selection, tree disclosure, copying, line numbers,
  highlighting, and responsive behavior.
- Use HighlightedCode directly for a single source value without a file tree.
- Use CodeEditor for editable code, debugger navigation, or completions.

## HighlightedCode

Status: ready

Import:

```tsx
import { HighlightedCode } from "@acton/ui"
```

Use HighlightedCode for read-only syntax-highlighted source code. It owns the
shared Shiki instance, JetBrains light/dark themes, loading fallback, scrolling,
and wrapping behavior.

### Languages

- tolk: Tolk source and generated declarations.
- func: legacy FunC source.
- tasm: TVM assembly and decompiled code.
- tlb: TL-B schemas.
- json: ABI, stack, metadata, and other JSON.
- Omit language for plain preformatted text with the same code geometry.

### Composition

```tsx
<RawDataBlock
  title="Disassembly"
  value={disassembly}
  customContent={
    <HighlightedCode value={disassembly} language="tasm" />
  }
/>
```

- value: complete source text to render.
- language: optional supported grammar.
- wrap: enables preformatted wrapping; disabled by default.
- maxHeight and minHeight: constrain the component-owned scroll area.
- className: allows a surface to override the documented
  --acton-highlighted-code-* sizing variables without creating another
  highlighter.

### Agent Guidance

- Keep fetching, decompilation, parsing, tabs, copy actions, and line annotations
  in caller-owned domain components.
- Compose HighlightedCode through RawDataBlock.customContent when code also
  needs a frame, title, copy button, or disclosure behavior.
- Use the Monaco-based CodeEditor for editable code, CodeLens, decorations,
  folding, trace navigation, or completion.
- A coverage viewer may use highlightCodeToTokens because it renders hit counts
  and status per line; do not flatten that viewer into a static block.
- Do not create local Shiki instances, theme observers, or hand-written token
  coloring.

## RawDataBlock

Status: ready

Import:

```tsx
import { RawDataBlock } from "@acton/ui"
```

Use RawDataBlock for large raw values and code-like payloads: base64, hex,
hashes, VM logs, disassembly output, raw message bodies, and state init. It
renders the scrollable `pre/code` area and owns copy button state.

### Composition

```tsx
<ContentTabs
  ariaLabel="Code formats"
  tabs={tabs}
  value={activeTab}
  onValueChange={setActiveTab}
>
  <RawDataBlock
    variant="embedded"
    value={activeValue}
    copyLabel={activeTab}
    wrap={activeTab !== "disasm"}
  />
</ContentTabs>
```

- `variant="embedded"`: use inside `ContentTabs` or another existing frame.
- `variant="standalone"`: draw a bordered raw-data panel.
- `wrap`: keep enabled for base64/hex payloads; set to `false` for disassembly,
  logs, and aligned preformatted output.
- `maxHeight`: caps the scroll region without requiring caller CSS.
- `copyLabel`: controls the accessible label and title for the copy button.
- `title` with `collapsible`: renders a compact header that expands/collapses
  the raw payload. Use `expanded`/`onExpandedChange` for controlled state or
  `defaultExpanded` for local state.
- `titleLabel`: accessible name for the collapse action when `title` is not
  plain text.
- `empty`: renders a quiet empty state instead of `pre/code`; copy is hidden.
- `emptyContent`: text or React content explaining why raw data is unavailable.
- `loading`: disables copy and renders a skeleton instead of ordinary content. A
  collapsible block hides its content and shows progress in the header action
  slot instead.
- `loadingLabel`: accessible name for the loading status.
- `children`: optional highlighted code fragments rendered inside the built-in
  `pre/code`; `value` remains the copy source.
- `customContent`: a complete pre-rendered code viewer that replaces the
  built-in `pre/code`. Use it when a highlighter already returns its own
  `<pre>` or structured HTML.

### States To Review Visually

- Embedded in ContentTabs
- Standalone framed panel
- Wrapped payload
- No-wrap preformatted output
- Collapsible title open and closed states
- Loading skeleton and collapsible header progress
- Empty data text and custom React content
- Copy hover/focus and copied state

### Agent Guidance

- Use RawDataBlock instead of local `pre`/`code` CSS for raw technical values.
- Keep tab state in ContentTabs and pass the active raw value into RawDataBlock.
- Use the built-in collapsible title instead of wrapping RawDataBlock in a
  one-off disclosure header.
- Use `empty` for missing VM logs, executor logs, state init, raw bodies, or
  other expected payloads. Do not display missing-data explanations as raw text.
- Use `loading` while raw data is being fetched. Do not render loading text as
  raw or empty content.
- Keep decoding, formatting, and syntax highlighting setup outside RawDataBlock.
- Do not use RawDataBlock for parsed key-value data, tables, or prose content.

## MarkdownText

Status: ready

Import:

```tsx
import { MarkdownText } from "@acton/ui"
```

Use MarkdownText for trusted markdown prose in help text, release notes,
inline technical notes, and agent-facing UI descriptions. It renders markdown
through `react-markdown` with GFM support instead of hand-parsing individual
syntax cases.

### Composition

```tsx
<MarkdownText tone="muted">
  {"Use `RawDataBlock` for VM logs and `DataTable` for structured values."}
</MarkdownText>
```

- `children`: trusted markdown source string.
- `tone`: `default` or `muted` text tone.
- `openLinksInNewTab`: adds external-link browser behavior when the caller
  wants markdown links to open in a new tab.
- `components`: optional `react-markdown` renderer overrides for known local
  elements.

### States To Review Visually

- Inline emphasis and inline code
- Links with default and new-tab policy
- Lists and task lists
- Fenced code blocks
- GFM tables
- Muted explanatory copy

### Agent Guidance

- Use MarkdownText when copy may contain backticked code, links, lists, or
  markdown tables.
- Keep it for prose and documentation-like text, not raw technical payloads.
- Use RawDataBlock for logs, base64, hex, disassembly, and code viewers.
- Do not hand-parse backticks, links, or markdown lists in app code when
  MarkdownText fits.
- Treat sanitization as a caller boundary concern; do not pass arbitrary
  untrusted user-authored markdown without a policy.

## Popover

Status: ready

Import:

```tsx
import { Popover } from "@acton/ui"
```

Use Popover for rich contextual overlays attached to inline values, status
labels, compact actions, or info triggers. It is the shared primitive for help
content that needs multiple lines, links, small actions, or structured detail.

### Composition

```tsx
<Popover
  ariaLabel="Send mode details"
  content={<SendModeHelp />}
  placement="top"
>
  <span>send mode 3</span>
</Popover>
```

- `content`: the overlay body. Keep it compact and caller-owned.
- `interaction`: `hover` by default, or `click` when users need to interact with
  links/buttons inside the panel.
- `placement`: preferred side: `top`, `right`, `bottom`, or `left`.
- Positioning uses Base UI collision handling. The preferred side can flip or
  shift to stay inside the viewport.
- `open`, `defaultOpen`, and `onOpenChange`: controlled or uncontrolled state.
- `openDelay` and `closeDelay`: tune hover timing when the host surface needs
  it.
- `contentClassName` and `triggerClassName`: local layout hooks; do not use them
  to replace the shared panel frame.
- `tabIndex`: defaults to `0` for text triggers. Pass `-1` when wrapping a
  focusable child such as `InlineButton`.

### States To Review Visually

- Hover inline technical value
- Click-triggered rich panel
- Interactive content with links/actions
- Preferred top/right/bottom/left placement
- Auto placement near viewport edges
- Keyboard focus and Escape close

### Agent Guidance

- Use Popover when contextual content must remain near the value or action it
  explains.
- Use hover for read-only inline help and click for interactive panels.
- Keep docs links, product copy, and domain-specific actions outside the
  component.
- Let Popover handle portal rendering and viewport positioning instead of local
  absolute-positioned panels.
- Do not put large workflows, destructive confirmations, or long forms inside a
  popover.
- Do not reuse the popover shadow on other components.

## InfoPopover

Status: ready

Import:

```tsx
import { InfoPopover } from "@acton/ui"
```

Use InfoPopover for the standard compact info icon attached to labels or
technical values. It composes Popover and only owns the trigger visual.

### Composition

```tsx
<InfoPopover id={descriptionId} ariaLabel="Show contract description">
  <ContractDescription />
</InfoPopover>
```

- `children`: compact help content rendered inside the popover panel.
- `id`: optional panel id for local aria relationships.
- `ariaLabel`: trigger and panel label. Defaults to `Show information`.
- `interaction`, `placement`, `open`, `defaultOpen`, `onOpenChange`,
  `openDelay`, and `closeDelay`: forwarded to Popover.
- Default placement is `right`.

### States To Review Visually

- Inline icon after a technical value
- Top/right placement near dense rows
- Interactive click content with a link or action
- Keyboard focus and Escape close

### Agent Guidance

- Use InfoPopover when the trigger should be the shared info icon.
- Use Popover directly when the trigger is custom text, a badge, or a button.
- Keep domain copy, docs links, and local actions in the caller.
- Do not rebuild icon help popovers with local portal or positioning code.

## Toast

Status: ready

Import:

```tsx
import { ToastProvider, useToast } from "@acton/ui"
```

Use Toast for temporary, non-blocking feedback after user actions and async
workflow changes. It is built on Base UI Toast and keeps the Acton API small:
wrap the app once, then call toast methods from action handlers.

### Composition

```tsx
function AppRoot() {
  return (
    <ToastProvider theme={theme}>
      <App />
    </ToastProvider>
  )
}

function RefreshButton() {
  const { showToast, updateToast } = useToast()

  async function refresh() {
    const toastId = showToast({
      title: "Refreshing wallets",
      description: "Fetching sessions and balances.",
      variant: "loading",
    })

    await refreshWallets()

    updateToast(toastId, {
      title: "Wallets refreshed",
      description: "Sessions and balances are up to date.",
      variant: "success",
    })
  }
}
```

- `ToastProvider`: app-level provider and viewport. Render it once near the
  root.
- `showToast(options)`: creates a toast and returns its id.
- Put the primary status text in `title`; reserve `description` for supporting
  detail, links, or error context.
- `updateToast(id, options)`: updates a toast in place. Use this for loading to
  success/error flows.
- `dismissToast(id?)`: closes one toast, or all toasts when called without an
  id.
- `promiseToast(promise, states)`: binds loading, success, and error states to a
  promise.
- `variant`: `info`, `success`, `error`, or `loading`.
- `durationMs`: custom auto-dismiss timeout. Base UI uses a non-auto-dismissed
  loading toast for promise/loading flows when appropriate.

### States To Review Visually

- Info
- Success
- Error
- Loading
- Loading updated to success/error
- Multiple stacked toasts
- Mobile viewport
- Keyboard focus and close button

### Agent Guidance

- Use Toast for copy, refresh, connection, approval, rejection, and recoverable
  error feedback.
- Use `updateToast` or `promiseToast` instead of stacking separate loading and
  completion messages.
- Keep text short and specific.
- Variants are expressed through icon color, border color, and text content.
  Do not add decorative side rails, status strips, or accent bars.
- Do not use Toast for field validation, destructive confirmations, long logs,
  forms, tables, or permanent page content.

## DataTable

Status: ready

Import:

```tsx
import {
  DataTable,
  DataTableBody,
  DataTableCell,
  DataTableGroupRow,
  DataTableHead,
  DataTableHeaderCell,
  DataTableRow,
  DataTableTable,
} from "@acton/ui"
```

Use DataTable for standalone framed tables in localnet, explorer, test UI, and
new Acton tools. It provides the shared card frame, optional title bar,
semantic table primitives, dense row styling, group rows, empty states, and
loading rows.

### Composition

```tsx
<DataTable title="Startup wallets" actions={<InlineButton>Refresh</InlineButton>} minWidth="54rem">
  <DataTableTable aria-label="Startup wallets">
    <DataTableHead>
      <DataTableRow>
        <DataTableHeaderCell>Name</DataTableHeaderCell>
        <DataTableHeaderCell>Address</DataTableHeaderCell>
        <DataTableHeaderCell align="right">Balance</DataTableHeaderCell>
      </DataTableRow>
    </DataTableHead>
    <DataTableBody>
      <DataTableRow hover>
        <DataTableCell tone="strong">deployer</DataTableCell>
        <DataTableCell truncate>{address}</DataTableCell>
        <DataTableCell align="right" tone="strong">100.2519 GRAM</DataTableCell>
      </DataTableRow>
    </DataTableBody>
  </DataTableTable>
</DataTable>
```

- `DataTable`: outer frame with optional `title`, `meta`, `actions`, and
  `minWidth`.
- `DataTableTable`: semantic table element with `layout="fixed"` by default.
- `DataTableHeaderCell`: header cell with optional `align`, `columnWidth`, and
  `truncate`.
- `DataTableCell`: body/footer cell with optional `align`, `tone`, `mono`,
  `truncate`, and `columnWidth`.
- `DataTableRow`: row primitive. Use `hover` for passive hover feedback,
  `interactive` for clickable rows, `selected` for the active row, and
  `groupChild` for rows revealed by a group row.
- `DataTableGroupRow`: full-width collapsible group row with chevron,
  `aria-expanded`, and caller-owned child rows.
- `DataTableEmpty`: full-width empty state row that preserves headers.
- `DataTableSkeletonRows`: repeated skeleton rows for loading table bodies.

### States To Review Visually

- Standalone table with title action
- Title metadata and row action
- Footer controls inside `DataTableFooter`
- Empty table body
- Loading rows with right-aligned skeleton cells
- Collapsible group row open and closed
- Horizontal overflow with stable column widths

### Agent Guidance

- Use DataTable instead of duplicating card/table CSS in feature code.
- Keep domain-specific rendering inside cells: address chips, badges, links,
  copy buttons, balance summaries, and formatted hashes.
- Use `minWidth` when technical columns need horizontal scrolling.
- Use `DataTableGroupRow` for collapsible table sections such as treasury
  deploys; do not hand-roll a div between table rows.
- Mark the rows revealed by that group with `DataTableRow groupChild` so the
  relationship is visible without changing the child row background.
- Use `DataTableSkeletonRows` for common repeated loading rows, or compose
  `Skeleton` inside cells for custom shapes.
- Use `DataTableEmpty` when there are no rows but the table headers still give
  useful context.
- Keep row click behavior in the caller. DataTable only provides visual row
  states and table semantics.

## Skeleton

Status: ready

Import:

```tsx
import { Skeleton, SkeletonText } from "@acton/ui"
```

Use Skeleton for loading placeholders when the destination layout is known but
data is still loading. It provides the shared shimmer, colors, reduced-motion
behavior, and basic placeholder shapes.

### Composition

```tsx
<Skeleton width="12rem" />
<Skeleton shape="rect" width="2.5rem" height="2.5rem" radius="md" />
<SkeletonText lineCount={4} widths={["68%", "100%", "84%", "52%"]} />
```

- `Skeleton`: a single placeholder. Use `shape="line"` for text-like values,
  `shape="rect"` for blocks/icons/cards, and `shape="circle"` for circular
  avatars or status dots.
- `SkeletonText`: repeated line placeholders with sensible default widths.
- `width` and `height`: local layout dimensions for the placeholder.
- `radius`: optional radius override for rectangular placeholders.
- `animated`: disables shimmer for static screenshots or special cases.

### States To Review Visually

- Single text line
- Rectangular block
- Circle/avatar
- Multi-line text group
- Table rows and repeated cards composed from Skeleton primitives

### Agent Guidance

- Use Skeleton instead of local shimmer keyframes.
- Compose skeleton rows, cards, and tables in feature CSS; keep only the
  placeholder shape in Skeleton.
- Use SkeletonText for code panels, raw data panels, and descriptions.
- Pair loading skeletons with an accessible status in the parent loading region
  when the loading state needs to be announced.
- Do not use Skeleton for empty states, error states, or hidden final content.

## Checkbox

Status: ready

Import:

```tsx
import { Checkbox } from "@acton/ui"
```

Use Checkbox for independent boolean choices in filters, settings, and option
lists. It renders a native checkbox input inside a clickable label and uses the
same 16px checked-box pattern as localnet-style filter controls.

### Composition

- `label`: required visible option text.
- `count`: optional number or compact value shown directly after the label, for
  example `Success 128` or `Failed 7`.
- `description`: optional helper text under the label for settings where the
  consequence is not obvious.

### States To Review Visually

- Unchecked
- Checked
- Disabled
- Disabled checked
- With count
- With description

### Agent Guidance

- Use Checkbox for independent on/off choices.
- Use `count` when the option describes a filtered set or status bucket.
- Use `description` sparingly; keep labels short and scannable.
- The check mark is drawn by Checkbox CSS; do not import an icon just to render
  the checked state.
- Do not use Checkbox for mutually exclusive choices; use radios or segmented
  controls.
- Do not use Checkbox as a command button.

## ThemeProvider and useTheme

Status: ready

Import:

```tsx
import { ThemeProvider, useTheme } from "@acton/ui"
```

Wrap each UI root once. ThemeProvider resolves the saved preference, falls back
to the system preference, persists changes, and synchronizes the theme markers
used by Acton tokens and legacy app styles. Theme toggles use the browser View
Transition API when available and fall back to an immediate switch elsewhere.

```tsx
<ThemeProvider storageKey="theme">
  <App />
</ThemeProvider>
```

Theme-aware components read the shared state instead of receiving theme props:

```tsx
const { theme, setTheme, toggleTheme } = useTheme()
```

- `storageKey`: local-storage key; defaults to `theme`.
- `defaultTheme`: `"light"`, `"dark"`, or `"system"`; defaults to `"system"`.
- `theme`: resolved `"light"` or `"dark"` theme.
- `setTheme`: sets an explicit resolved theme.
- `toggleTheme`: switches between light and dark.

### Agent Guidance

- Use one ThemeProvider at the application root.
- Read theme state with `useTheme`; do not inspect DOM classes or local storage
  from feature components.
- Do not pass theme through app, page, sidebar, toast, or syntax-highlighting
  props.

## ThemeSwitch

Status: ready

Import:

```tsx
import { ThemeProvider, ThemeSwitch } from "@acton/ui"
```

Use ThemeSwitch for app-level light/dark mode toggles in app chrome, sidebars,
and settings surfaces. It preserves the existing shared-ui segmented pill
appearance while using Acton tokens for border, text, active surface, spacing,
and focus treatment.

### Composition

```tsx
<ThemeProvider>
  <ThemeSwitch />
</ThemeProvider>
```

- ThemeSwitch reads the current theme and toggle action from ThemeProvider.
- `theme` and `onToggleTheme` remain available only for controlled visual-state
  previews, such as the component gallery.
- `data-theme-toggle`: emitted for compatibility with existing app chrome CSS.

### States To Review Visually

- Light active
- Dark active
- Toolbar placement
- Keyboard focus

### Agent Guidance

- Use ThemeSwitch for global app theme changes.
- Keep the Sun/Moon segmented pill appearance; do not rebuild it with Button.
- Its default accessible label describes the target theme; override it only
  when surrounding context requires different wording.
- Do not use ThemeSwitch for local display modes such as source/trace view;
  use a segmented control for those.
