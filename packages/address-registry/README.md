# @acton/address-registry

Unified TON address metadata from multiple community-maintained sources.

This package will aggregate and normalize data from:

- [tonkeeper/ton-assets](https://github.com/tonkeeper/ton-assets)
- [catchain/address-book](https://github.com/catchain/address-book)

The generator downloads the account YAML files from both upstream repositories,
validates their shapes, normalizes addresses to raw form, merges equal entries,
and applies manual conflict resolutions. It fails if any name conflict does not
have a resolution, then writes `src/addresses.json`. The stable TypeScript
binding lives in `src/addresses.ts`:

```sh
bun run generate
```
