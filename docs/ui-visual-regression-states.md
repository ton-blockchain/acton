# Localnet and Explorer visual regression states

This document defines the visual-regression contract for `acton-localnet-ui` and
`acton-explorer-ui`. It turns “all possible states” into a finite, reviewable matrix and
keeps the suite splittable across independent workers or agents.

## Goals

- Catch layout, typography, token, responsive, overlay, and interaction regressions
- Run without a live TON endpoint by default
- Keep scenarios deterministic: fixed time, locale, theme, viewport, storage, and API fixtures
- Give every screenshot a stable scenario ID and a single owning spec file
- Allow ten contributors to add coverage concurrently without editing shared files

The suite does not try to snapshot every combination. It covers every distinct renderer and
state transition, then applies theme and viewport variants only where they can change layout.

## Scenario model

Every scenario is the product of five axes:

| Axis | Values |
| --- | --- |
| Surface | Route or named overlay/component state |
| Data | loading, empty, populated-minimal, populated-dense, partial, error, unauthorized |
| Interaction | idle, hover, focus, expanded, selected, filtered, modal/popover open |
| Viewport | desktop 1440×1000, narrow 980×900, mobile 390×844 |
| Theme | light, dark |

Rules:

1. Every route gets desktop/light coverage for its meaningful data states
2. Dark coverage is required for each distinct page family, not every data permutation
3. Narrow/mobile coverage is required where navigation, tables, grids, graphs, code, or overlays reflow
4. Hover/focus/open screenshots are required only for interactions that reveal different UI
5. Dynamic values use fixed fixtures or `data-visual-dynamic` placeholders
6. A scenario ID never changes after its baseline is accepted

Scenario IDs use `<app>-<area>-<state>-<variant>`, for example
`exp-account-populated-holders-dark` or `loc-wallets-empty-mobile`.

## Common cross-cutting states

These states apply to both applications where the surface supports them:

- Initial loading and delayed loading/skeleton
- Empty result and filtered-empty result
- Minimal populated result and dense/overflowing result
- Partial response where one panel succeeds and another fails
- Recoverable API error and retry
- Unauthorized API response
- Long labels, addresses, hashes, token symbols, and unbroken code lines
- Hover, keyboard focus, selected row/tab, expanded/collapsed content
- Toast success, warning, and error
- Modal/popover/dropdown open, validation error, and dismiss state
- Light/dark theme
- Desktop/narrow/mobile viewport
- Reduced-motion rendering and disabled animation at capture time

## Localnet UI inventory

| Area / route | Required states | Responsive or interaction variants |
| --- | --- | --- |
| Shell and navigation | expanded, collapsed, edge preview, active item, disconnected, API token set | narrow nav, mobile menu, dark |
| `/dashboard` | loading, empty localnet, populated accounts, partial metadata, API error | dense accounts, long names, dark, mobile |
| `/faucet` | initial form, wallet selected, pending, success, validation error, API error | focused fields, toast, mobile |
| `/wallets` | loading, empty, populated, imported, API error | create/import dialogs, dense list, mobile |
| `/tokens` | loading, empty, populated, long metadata, API error | table overflow, mobile, dark |
| `/nfts` | loading, empty, populated, missing image/metadata, API error | table overflow, mobile, dark |
| `/api-calls` | loading, empty, populated, expanded request/response, failed call | filtering, copy action, long JSON, mobile |
| `/api-reference/v2` | loaded reference, loading, spec error, unauthorized | operation expanded, try-it form, dark |
| `/api-reference/v3` | loaded reference, loading, spec error, unauthorized | operation expanded, try-it form, dark |
| `/api-reference/control` | loaded reference, loading, spec error, unauthorized | operation expanded, try-it form, dark |
| `/explorer` | search landing, search focus, invalid query | collapsed navigation, mobile, dark |
| `/explorer/blocks` | loading, empty, populated, pagination, API error | narrow table, mobile, dark |
| `/block/:workchain/:shard/:seqno` | loading, populated, no transactions, API error | long hashes, mobile, dark |
| `/explorer/abi` | loading catalog, populated, filtered, filtered-empty, catalog error | dense cards/table, mobile, dark |
| `/explorer/abi/:slug` | known ABI, unknown ABI, long definitions | tabs/sections expanded, mobile, dark |
| `/explorer/sources` | empty registry, populated, filtered, filtered-empty, storage error | long paths, mobile, dark |
| `/explorer/favorites` | empty, loading, populated, partial API failure | dense list, mobile, dark |
| `/explorer/address/:address` | see shared account matrix below | all account variants |
| `/explorer/tx/:hash` | see shared transaction matrix below | all transaction variants |
| `/explorer/tx/:hash/trace` | retrace loading, complete, incomplete, failed | modal/panel states, dark, narrow |
| Localnet auth overlay | optional token, required token, rejected saved token, filled token | validation, close disabled, mobile, dark |

## Explorer UI inventory

| Area / route | Required states | Responsive or interaction variants |
| --- | --- | --- |
| Shell/header | default mainnet, testnet, custom network, network menu open | compact header, mobile, dark |
| Custom network form | add empty, validation error, populated, edit, delete confirmation/result | API key present, toast, mobile |
| `/` | search landing, focused search, invalid/unknown query | mobile, dark |
| `/blocks` | loading, empty, populated, pagination, API error | narrow table, mobile, dark |
| `/block/:workchain/:shard/:seqno` | loading, populated, no transactions, API error | long hashes, mobile, dark |
| `/abi` | loading catalog, populated, filtered, filtered-empty, catalog error | mobile, dark |
| `/abi/:slug` | known ABI, unknown ABI, long definitions | mobile, dark |
| `/sources` | empty registry, populated, filtered, filtered-empty, storage error | long paths, mobile, dark |
| `/favorites` | empty, loading, populated, partial API failure | dense list, mobile, dark |
| `/address/:address` | see shared account matrix below | all account variants |
| `/tx/:hash` | see shared transaction matrix below | all transaction variants |
| `/tx/:hash/trace` | retrace loading, complete, incomplete, failed | modal/panel states, dark, narrow |

## Shared account matrix

Account coverage is shared because explorer-ui consumes the explorer implementation from
localnet-ui. Both hosts still receive shell-level snapshots.

- Account loading, not found, invalid address, API error, and populated
- Active, uninitialized, frozen, and non-existing account states
- Plain contract, named contract, wallet, Jetton master/wallet, NFT collection/item
- Zero and large balances; long and localized values
- History: empty, populated, incoming/outgoing, failed, filtered, date filter open
- History filters: minimum GRAM value, hide token transfers, include selected token masters
- Tokens: empty, one, dense, broken metadata/image
- Holders: empty, one, dense; owner/wallet/value last column aligned right
- Contract: no ABI, ABI only, source only, verified source, long code, file tree collapsed
- Favorite off/on, editable label, copy actions, address popover
- Desktop/narrow/mobile and one representative dark state per tab

## Shared transaction matrix

- Loading, not found, invalid hash, API error
- Successful and failed transaction; complete and incomplete trace
- Incoming/outgoing/internal/external/bounced messages
- Compute/action/storage phases present, absent, failed, and collapsed/expanded
- Parsed body unavailable, scalar fields, nested structs, arrays, maps, cells, addresses, nulls
- Storage diff: add/remove/change object fields, map entries, array entries, empty containers
- Actions: reserve, send message, change library, unknown action, no actions
- Value flow: hidden/shown, zero/positive/negative values, multiple traces, row separators off
- Fee summary: minimal, dense, treasury deploys collapsed/expanded, linked row hover
- Verified code: tree open/collapsed, tree hidden, long file, horizontal/vertical scrolling
- Retrace: idle, running, complete, incomplete warning, failure, logs collapsed/expanded
- Popovers, copy actions, address/contract click targets, and keyboard focus
- Desktop/narrow viewport and representative dark states

## Deterministic localnet snapshot contract

Data scenarios use a real Acton localnet loaded from
`ui-e2e/fixtures/localnet/ui-state.json`. They do not replace Toncenter responses with browser
mocks. This exercises the real v2/v3 serializers, metadata registries, transaction shapes, and
control endpoints.

- `acton-localnet-ui` keeps browser requests same-origin and points its Vite proxy at the fixture node
- `acton-explorer-ui` remains a standalone Toncenter client; deterministic scenarios select a
  custom network whose v2/v3 URLs point at the same fixture node
- Mainnet/Testnet shell scenarios may render without a node, but baseline data scenarios never
  depend on changing public-chain data
- The node starts with `--no-mining` so block timestamps and contents remain stable
- Loading screenshots delay the real API request in the owning Playwright page before forwarding
  it to the node. This keeps the response real and the delay worker-local. Tests with a dedicated
  node may instead set `response_delay_ms` through `/acton_setNetworkConditions`
- Error and unauthorized states use real localnet controls/configuration or a dedicated node,
  not response-shape mocks
- Mutating scenarios run on a worker-local node or revert to the canonical snapshot before capture
- Browser storage is cleared before each scenario; custom networks, favorites, and address-book
  state are installed explicitly by the owning spec

## Parallel ownership model

The suite is partitioned into ten shards. A contributor edits only its spec directory,
fixtures, and snapshots. Shared support is changed by a designated harness owner.

| Shard | Ownership |
| --- | --- |
| 01 | localnet shell, navigation, auth, themes, responsive shell |
| 02 | localnet dashboard and API calls |
| 03 | localnet faucet and wallets |
| 04 | localnet tokens and NFTs |
| 05 | explorer shell, networks, landing, search, favorites |
| 06 | blocks and block details in both hosts |
| 07 | ABI and source catalog/details in both hosts |
| 08 | account overview, history, tokens, holders, contract |
| 09 | transaction overview, phases, parsed values, storage diff |
| 10 | transaction actions, value flow, code viewer, retrace, logs |

Shared files are frozen during a parallel batch:

- `playwright.config.ts`
- `e2e/support/**`
- this document’s conventions

New scenarios are appended to the owning shard’s manifest. Snapshot paths include app,
spec filename, and scenario ID, so separate workers never write the same file. Read-only scenarios
can share the immutable node with `--workers=10`; mutating scenarios receive worker-local ports.
Authoring can use ten agents with the same ownership split.

## Acceptance criteria

A scenario is complete only when it has:

1. A stable scenario ID in its spec
2. Deterministic real-node snapshot coverage with no changing public-chain dependency
3. Semantic assertions proving the intended state was reached
4. A screenshot after fonts, animations, focus, and dynamic text are stabilized
5. A baseline in both required host apps when host chrome differs
6. Passing lint/typecheck and Playwright execution

The initial implementation starts with shell/static routes and shared harness behavior.
Network-heavy matrices should be added shard-by-shard using the same snapshot contract.
