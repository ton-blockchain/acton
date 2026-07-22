# Acton UI Design Rules

This document defines the visual and implementation rules for `@acton/ui`.
It is written for both humans and coding agents. It is not prop documentation;
use TypeScript source for exact APIs and `COMPONENTS.md` for the component
inventory.

## Goals

- Keep future Acton UIs visually consistent across localnet, explorer, test UI,
  and new tools.
- Prefer quiet, dense, shadcn-ish application UI over decorative layouts.
- Make component choices predictable so agents do not invent one-off styles.
- Keep the package generic: no TON domain rendering, no explorer-only layouts,
  no test-report-specific logic.

## Token Rules

- Use `src/styles/tokens.css` for all shared color, spacing, typography, radius,
  focus, and timing decisions.
- Color tokens use `hsl(...)`. Do not add hex, `rgb(...)`, or `rgba(...)` color
  values in component or gallery CSS.
- Every new token must have a short comment explaining its role.
- Prefer semantic tokens over literal intent:
  - Use `--acton-color-text`, not "black".
  - Use `--acton-color-surface-hover`, not "gray hover".
  - Use `--acton-color-danger-text` for destructive inline text/icons.
- Do not use shadows for controls, cards, focus, or elevation. Use borders,
  surfaces, and outline focus treatment. `Popover` is the current exception:
  floating overlays may use `--acton-shadow-popover` so they separate from the
  page.
- Never use decorative side rails, status strips, or accent bars anywhere in
  the system. State must be communicated through content, icons, semantic
  border/fill tokens, or explicit structure instead.
- Use existing spacing tokens where possible. Add a token only when the spacing
  value is expected to recur.

## Theme Rules

- Components must work in light and dark themes through tokens.
- Theme is applied by an ancestor with `data-theme="dark"` or the
  `dark-theme` class on `:root`.
- Do not use `dark:`-style one-off selectors in component CSS.
- Primary buttons are inverted neutral:
  - Light theme: dark fill, light text.
  - Dark theme: light fill, dark text.
- Destructive filled controls and destructive inline text use separate tokens.
  Do not force one danger color to serve both jobs.

## Component Rules

- Components live under `src/components/<ComponentName>/`.
- Each component has:
  - `<ComponentName>.tsx`
  - `<ComponentName>.module.css`
  - `index.ts`
- Export new components from `src/components/index.ts` and `src/index.ts`.
- Add package subpath exports when the component is intended to be imported
  directly.
- Use CSS Modules. Do not introduce Tailwind classes inside `@acton/ui`.
- Compose class names with `cx(...)`.
- React target is React 19. Use `ComponentPropsWithRef<...>` and accept `ref`
  as a normal prop. Do not add `forwardRef`.
- Keep comments rare and only for non-obvious behavior.

## Current Primitives

### Button

Use `Button` for boxed command actions: submit, confirm, create, refresh,
delete, start, stop.

- `primary`: one main action per local decision area.
- `secondary`: default neutral action.
- `outline`: low-emphasis action with visible boundary.
- `ghost`: low-emphasis toolbar or repeated action.
- `danger`: destructive boxed action.

Do not use `Button` for route navigation or inline command links.

### Breadcrumbs

Use `Breadcrumbs` for compact page hierarchy and explorer paths.

- It owns breadcrumb nav/list semantics, separators, link/current styling,
  truncation, and partial loading segments.
- It does not own routing. Pass router integration through the item `link`
  callback.
- It does not format domain values. Format addresses, hashes, block labels, and
  ABI names before passing them as labels.
- Set `truncate: false` on stable short labels like `Explore`, `Blocks`, and
  `Accounts`.
- Set `truncate: "middle"` on long string/number technical values where the
  suffix should remain visible, such as addresses and hashes. Middle truncation
  is deterministic (`prefix...suffix`), not measured by JavaScript.
- Use item-level `loading` and `skeletonWidth` when only part of the breadcrumb
  path is still loading.
- Keep stable segments visible during loading instead of replacing the entire
  breadcrumb row with a skeleton.
- Use it only for ordered ancestors of the current page.

### InlineButton

Use `InlineButton` for visible text actions embedded in dense content, for
example `Debug`, `Inspect`, or `Reveal`.

- It has no boxed surface.
- Use `utility` for micro-sized text+icon commands such as `Copy raw body`,
  `Copy raw state init`, and similar raw-data helpers.
- Keep `utility` smaller than normal inline actions; it should read like an
  inline caption action near dense technical text.
- `utility` uses a lighter 500 text weight so it does not look like a regular
  command button.
- It can use `accent` or `danger` text tokens.
- It is not a replacement for icon-only actions.

### InlineActions

Use `InlineActions` when inline content needs icon-only actions such as copy,
open, or remove.

- Put the value/content in `children`.
- Put buttons in the `actions` slot.
- Use `visibility="hover"` for helper copy/open actions.
- Use `visibility="always"` for remove actions, important row actions, and touch
  contexts.
- `InlineAction` is visually neutral. Do not add a danger variant for tiny
  inline icons.
- Use `CopyInlineAction` when copy should change to a check mark after click.
- Keep actions 20px controls with 13px icons to match existing inline values in
  localnet/shared UI.
- Keep hover reveal keyboard-accessible via `focus-within`.

### DisclosureToggle

Use `DisclosureToggle` for compact inline Show/Hide controls that reveal content
below or nearby, for example parsed body, state init, disassembled code,
storage diffs, and action details.

- It owns `button`, `aria-expanded`, optional `aria-busy`, chevron direction,
  and visible state text.
- Keep stable labels and values outside the component for precise layout
  compatibility. Use external text for `Parsed Body`, `State Init`, `Code`, and
  Total Actions counts.
- Do not add label/value props to DisclosureToggle. Local layouts own labels,
  values, and spacing around the trigger.
- Use `loading` and `loadingLabel` for async reveal actions such as
  `Load`/`Loading`.
- Keep expanded content outside the component.
- Do not use it for full-width disclosure headers, accordions, or form reveal
  buttons.

### ContentTabs

Use `ContentTabs` for connected tabs above a bordered content panel, for example
disasm/base64/hex viewers, parsed data variants, trace sections, and compact
tables that switch inside the same visual frame.

- It owns tablist/tab/tabpanel semantics, selected state wiring, and keyboard
  navigation.
- It does not own the content. Pass the active panel as `children`.
- Hover changes only tab text color; selected state owns the connected panel
  background.
- Keep it controlled with `value` and `onValueChange`.
- `onValueChange` may return a Promise. ContentTabs immediately selects the
  target tab and shows a loading fallback until the Promise settles and
  controlled `value` catches up.
- Use `loading` and `loadingValue` when async loading is controlled outside the
  tab change handler.
- The default loading fallback is `SkeletonText`; use `loadingFallback` with
  `Skeleton`/`SkeletonText` for exact local shapes.
- Use `panelClassName` for code scroll regions, tables, local padding, or other
  content-specific sizing.
- Keep domain parsing, decoding, formatting, and data loading outside the
  component.
- Do not use it for standalone segmented controls without a connected panel.
- Do not use it when tabs do not share the same framed content area.

### PillTabs

Use `PillTabs` for detached pill-like selector rows, for example Test UI trace
selection, compact item filters, and selectors with a collapsible group summary.

- It owns the horizontal row, gap, overflow behavior, selected pill styling,
  disabled styling, and group toggle chevron.
- Use `PillTabToggle` for group summaries such as treasury deploy traces. It
  expands or collapses a group and is not the selected tab itself.
- Use `PillTab variant="muted"` for group children, skipped items, or
  low-emphasis trace tabs.
- Keep selected item state, group filtering, and domain labels in the caller.
- Do not use it for connected panels; use `ContentTabs` instead.

### VisuallyGroupedNumber

Use `VisuallyGroupedNumber` for long decimal technical values where readability
needs visual grouping but the text must stay exact for copying and inspection.

- It visually separates decimal groups with CSS; it does not insert real
  separators into the value.
- It only groups plain decimal strings, including negative and fractional
  values. Hex, hashes, addresses, base64, and short values render unchanged.
- Keep typography, color, and table/cell layout in the caller.
- Keep domain-specific formatting outside the component.
- Do not use it for truncation. Use a dedicated truncation component or local
  formatting for addresses and hashes.

### RawDataBlock

Use `RawDataBlock` for large raw values and code-like payloads: base64, hex,
hashes, VM logs, disassembly output, raw message bodies, and state init.

- It owns `pre/code` layout, max-height scrolling, wrap/no-wrap behavior, and
  copy button state.
- Use `variant="embedded"` inside `ContentTabs` so the tab panel remains the
  only visible frame.
- Use `variant="standalone"` when the raw value needs its own bordered panel.
- Use `title` with `collapsible` when the raw value needs a compact reveal
  header. Prefer the built-in header over wrapping the block in another
  disclosure component.
- The header copy action remains available while the content is collapsed.
- Use `empty` with `emptyContent` when the raw value was expected but is not
  available. Do not render absence messages as fake raw data inside `pre`.
- Use `wrap={false}` for disassembly, VM logs, and aligned preformatted output.
- Keep decoding, tab state, syntax highlighting setup, and domain labels outside
  the component.
- Pass `copyLabel` for useful accessible copy labels.
- Do not write local `pre`/`code` panel CSS for raw values when this component
  fits.

### MarkdownText

Use `MarkdownText` for trusted markdown prose in product notes, help text,
release notes, and agent-facing UI descriptions.

- It owns shared markdown typography, links, inline code, lists, task lists,
  blockquotes, code fences, and GFM tables.
- Use `tone="muted"` for secondary explanatory text that still needs markdown
  support.
- Use `openLinksInNewTab` only when the markdown intentionally points outside
  the current app.
- Use `components` overrides for known markdown elements when a local surface
  needs custom rendering.
- Keep sanitization decisions at the caller boundary. Do not feed arbitrary
  untrusted markdown into the component without a sanitization policy.
- Do not use it for raw payloads, VM logs, base64, hex, disassembly, or code
  viewers. Use `RawDataBlock` for those.

### Popover

Use `Popover` for contextual overlays attached to inline values, status labels,
and compact actions when the explanation needs rich content, links, or local
interactive controls.

- It owns portal rendering, outside click, Escape close, focus/hover behavior,
  viewport positioning, and automatic side fallback through Base UI.
- Use hover interaction for lightweight inline explanations.
- Use click interaction when the panel contains links, buttons, or content users
  need to inspect deliberately.
- Keep domain copy, docs links, and action wiring in the caller.
- Use `placement` as a preference; Base UI may shift or flip the panel to keep
  it inside the viewport.
- `Popover` is allowed to use `--acton-shadow-popover`. Do not copy that shadow
  token into controls, cards, tables, or other framed surfaces.
- Keep popover content compact. If the content becomes a workflow, use a page
  region or modal-level component instead.

### InfoPopover

Use `InfoPopover` for the standard compact info icon attached to labels or
technical values.

- Build it from `Popover`; do not duplicate portal, scroll, resize, or
  positioning logic.
- Use it when the trigger should be the shared info icon. Use `Popover`
  directly for custom text, badges, buttons, or other triggers.
- Keep the help content caller-owned and compact.
- Use click interaction when the panel contains links or actions users need to
  reach deliberately.

### Toast

Use `Toast` for short-lived, non-blocking feedback after user actions and async
workflows.

- Wrap app roots in `ToastProvider` once and call `useToast` from action
  handlers.
- Use `showToast` for immediate feedback such as copied values, refresh
  completion, connection changes, and recoverable errors.
- Put the primary status in `title`. Use `description` only for supporting
  detail, recovery context, links, or technical error text.
- Use `updateToast` when a loading state resolves into success or failure.
  Keep the same toast id instead of stacking separate loading and completion
  messages.
- Use `promiseToast` when the workflow already returns a promise and loading,
  success, and error messages can live together.
- Keep descriptions compact. Toasts may contain a short link or inline code, but
  not logs, tables, forms, or large raw payloads.
- Do not add decorative side rails, status strips, or accent bars. Variants are
  expressed through icon color, border color, and text content only.
- Do not use toast for destructive confirmations or field validation. Use a
  dialog or inline form feedback for those.

### DataTable

Use `DataTable` for standalone framed data tables in localnet, explorer, test
UI, and future Acton tools.

- It owns the framed shell, optional title bar, title metadata/actions,
  semantic table primitives, dense row sizing, separators, empty state rows, and
  shared skeleton rows.
- Keep domain rendering inside cells: address chips, hash formatting, links,
  badges, wallet summaries, and copy actions are caller-owned.
- Use `DataTable` `title`, `meta`, and `actions` for card-table headers such as
  `Startup wallets` with `Refresh`, or `Sessions` with pending approval count.
- Use `DataTableGroupRow` for collapsible row groups such as treasury deploys
  in trace fee tables. It owns the full-width group row, chevron, and
  `aria-expanded`; callers own which child rows render.
- Mark rows revealed by a group with `DataTableRow groupChild`. This adds a
  small left rail without shifting cell content or changing the row background.
- Use `DataTableSkeletonRows` for repeated loading rows. For highly custom
  skeleton cells, compose `Skeleton` inside `DataTableCell`.
- Use `DataTableEmpty` when the empty state should preserve the table frame and
  headers.
- Prefer `DataTableRow hover` for passive hover feedback and
  `DataTableRow interactive` only when the row is actually clickable.
- Set `minWidth` on `DataTable` when columns need horizontal scrolling instead
  of squeezing technical values.
- Use `DataTableHeaderCell columnWidth` for stable fixed-layout columns rather
  than width classes when the width belongs to the table contract.
- Keep footer forms and custom controls in `DataTableFooter` with a full-width
  cell when they belong to the table data flow.

### Skeleton

Use `Skeleton` for shared loading placeholders.

- Use `Skeleton` for a single line, block, icon, avatar, or status pill
  placeholder.
- Use `SkeletonText` for repeated line placeholders in code, raw data, details,
  and text panels.
- Compose table rows, cards, and page-level loading layouts locally from
  Skeleton primitives. Do not create feature-specific skeleton variants inside
  `@acton/ui` unless the shape becomes broadly reusable.
- The shimmer, colors, reduced-motion behavior, and base radii belong to
  Skeleton. Do not duplicate local shimmer keyframes.
- Skeleton should not represent empty or error states.

### Checkbox

Use `Checkbox` for independent boolean choices in filters and settings.

- Use `count` for status buckets or filtered sets.
- Use `description` only when the label is not enough.
- The check mark is drawn by CSS; do not import an icon just for the checked
  state.
- Active unchecked labels use normal text color. Disabled state must be
  visibly different through opacity and cursor.

### ThemeSwitch

Use `ThemeSwitch` for global light/dark theme changes.

- Preserve the existing shared-ui segmented pill appearance.
- Use Sun/Moon icons from the standard icon library.
- Keep `data-theme-toggle` for compatibility with app chrome CSS.
- Do not rebuild it with `Button`.
- Do not use it for local view modes; use a segmented control for those.

## Icon Rules

- Use `lucide-react` for standard app icons when a component owns its icon
  internally or when gallery examples need icons.
- For components that accept arbitrary icons, prefer passing lucide icons from
  the caller rather than hard-coding product-specific artwork.
- Icon-only buttons must have an accessible `label` or `aria-label`.
- Inline action icons should stay compact: 13px inside a 20px control unless a
  component has a specific reason to differ.

## Interaction Rules

- Hover and focus states should change color or surface, not move the element.
- Do not animate vertical movement on hover.
- Focus states use `outline`; do not use focus shadows.
- Hover-only affordances must also appear on keyboard focus.
- On coarse pointer devices, hover-revealed inline actions should be visible.
- Avoid layout shift when actions appear or state changes.

## Gallery Rules

- Every component added to `@acton/ui` should also be added to
  `@acton/ui-gallery`.
- The gallery should show practical visual states, not exhaustive prop docs.
- Each gallery entry needs:
  - usage guidance
  - avoid guidance
  - agent summary
  - import statement
  - all important visual variants and states
- Keep gallery content centered with the existing 1200px layout.
- Gallery examples may use realistic technical values such as addresses, hashes,
  routes, and status counts.

## Documentation Rules

- Update `COMPONENTS.md` when adding or changing a component contract.
- Update this file when a new reusable visual rule is established.
- Do not document unrelated future ideas as if they already exist.
- Keep docs direct and operational. Prefer "Use X when Y" over abstract design
  language.

## Avoid

- Raw color literals in CSS.
- Shadows.
- One-off component classes in application code when a primitive exists.
- Rebuilding `Button`, `Breadcrumbs`, `InlineButton`, `InlineActions`,
  `DisclosureToggle`, `ContentTabs`, `PillTabs`, `Popover`, `RawDataBlock`,
  `Toast`, `MarkdownText`, `DataTable`, `Skeleton`, `Checkbox`, or
  `ThemeSwitch` with ad hoc markup.
- Shadows outside the `Popover` floating overlay exception.
- Decorative side rails, status strips, or accent bars anywhere in the system.
- Adding variants because a single screen wants a custom look.
- Mixing several high-emphasis actions in one compact group.
