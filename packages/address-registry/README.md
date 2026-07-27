# @acton/address-registry

Unified TON address metadata from multiple community-maintained sources.

This package will aggregate and normalize data from:

- [tonkeeper/ton-assets](https://github.com/tonkeeper/ton-assets)
- [catchain/address-book](https://github.com/catchain/address-book)

The generator currently implements the source-reading stage. It downloads the
account YAML files from both upstream repositories, validates their shapes, and
reports how many entries were read:

```sh
bun run generate
```

The upstream `accounts/scammers.yaml` and `source/scam.yaml` files are
intentionally excluded. The generator does not normalize addresses, resolve
name conflicts, or write a generated registry yet.
