# @acton/address-registry

Unified TON address metadata from multiple community-maintained sources.

This package will aggregate and normalize data from:

- [tonkeeper/ton-assets](https://github.com/tonkeeper/ton-assets)
- [catchain/address-book](https://github.com/catchain/address-book)
- the local Acton address list

Add Acton-maintained addresses to the matching network list in
`scripts/sources/acton.ts`:

```ts
export const ACTON_MAINNET_ADDRESSES = [
  {
    address: "EQ...",
    name: "Acton service",
  },
] as const satisfies readonly SourceAddress[]

export const ACTON_TESTNET_ADDRESSES = [
  {
    address: "kQ...",
    name: "Acton testnet service",
  },
] as const satisfies readonly SourceAddress[]
```

The generator downloads the account YAML files from both upstream repositories,
validates all three sources, splits friendly addresses by network, normalizes
them to raw form, merges equal entries, and applies manual conflict resolutions.
Raw addresses, which do not encode a network, default to mainnet. It fails if
any name conflict does not have a resolution, then writes the intermediate
`src/mainnet-base.json` and `src/testnet-base.json`:

```sh
bun run generate:base
```

The network generator reads both base files, copies `mainnet-base.json` to the
complete `src/mainnet.json`, adds mainnet registry entries whose accounts are
active on testnet, and writes the complete `src/testnet.json`:

```sh
bun run generate:networks
```

Set `TONCENTER_API_KEY` to avoid the unauthenticated request delay. The check is
independent from the source generator. Explicit entries from
`src/testnet-base.json` take precedence over discovered mainnet entries with equal
raw addresses. The stable TypeScript binding reads the complete generated registries
from `src/addresses.ts`.
