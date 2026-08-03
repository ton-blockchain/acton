# @acton/address-registry

Unified TON address metadata from multiple community-maintained sources.

This package will aggregate and normalize data from:

- [tonkeeper/ton-assets](https://github.com/tonkeeper/ton-assets)
- [catchain/address-book](https://github.com/catchain/address-book)
- the local Acton address list

Add Acton-maintained addresses to `scripts/sources/acton.ts`:

```ts
export const ACTON_ADDRESSES = [
  {
    address: "EQ...",
    name: "Acton service",
  },
] as const satisfies readonly SourceAddress[]
```

The generator downloads the account YAML files from both upstream repositories,
validates all three sources, splits friendly addresses by network, normalizes
them to raw form, merges equal entries, and applies manual conflict resolutions.
Raw addresses, which do not encode a network, default to mainnet. It fails if
any name conflict does not have a resolution, then writes `src/mainnet.json` and
`src/testnet.json`. The stable TypeScript binding lives in `src/addresses.ts`:

```sh
bun run generate
```
