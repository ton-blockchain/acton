# @acton/ui

Base UI kit for Acton web surfaces.

This package is intentionally small at first. It owns reusable visual primitives,
shared design tokens, and low-level helpers that future Acton UIs can depend on
without pulling in TON-specific rendering logic.

## Scope

Belongs here:

- Design tokens and theme primitives
- Generic controls such as Button, Input, Badge, Tabs, Dialog, Popover
- Small helper utilities used by UI primitives

Does not belong here:

- Transaction rendering
- TON domain types
- Explorer-specific layouts
- Test report or coverage-specific UI

## Usage

```tsx
import {
  Button,
  Checkbox,
  CopyInlineAction,
  InlineAction,
  InlineActions,
  InlineButton,
  ThemeSwitch,
} from "@acton/ui"
import "@acton/ui/styles/tokens.css"
```

## Component Catalog

Use [COMPONENTS.md](./COMPONENTS.md) as the text inventory for humans and coding
agents. Use `@acton/ui-gallery` for visual review of variants, states, and usage
guidance.
