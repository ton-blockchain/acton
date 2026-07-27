# @acton/address-registry

Unified TON address metadata from multiple community-maintained sources.

This package will aggregate and normalize data from:

- [tonkeeper/ton-assets](https://github.com/tonkeeper/ton-assets)
- [catchain/address-book](https://github.com/catchain/address-book)

The generator downloads the account YAML files from both upstream repositories,
validates their shapes, normalizes addresses to raw form, merges equal entries,
applies manual conflict resolutions, and saves unresolved name conflicts to
`conflicts.json`:

```sh
bun run generate
```

The upstream `accounts/scammers.yaml` and `source/scam.yaml` files are
intentionally excluded. The generator does not write a generated registry yet.
