# NFT

This package implements the reference NFT collection and item contracts. Tests cover collection behavior and item behavior in focused reference suites.

Scripts in `scripts/` cover deployment and collection or item management:

- `deployCollection.tolk` — deploys an NFT collection with on-chain metadata and royalty params.
- `deployItem.tolk` — mints a single NFT item into an existing collection.
- `deployBatch.tolk` — batch-mints multiple items in a single transaction.
- `transferItem.tolk` — transfers an NFT item to a new owner.
- `changeAdmin.tolk` — changes the admin address of an existing collection.

Run them with `acton script scripts/<name>.tolk`.
