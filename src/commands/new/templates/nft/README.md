# NFT Template

This project was generated from Acton's `nft` template. It includes an NFT
collection contract, an NFT item contract, wrappers, focused test suites, and
ready-to-run deployment and management scripts.

## What Is Included

- `contracts/NftCollection.tolk` implements the NFT collection.
- `contracts/NftItem.tolk` implements individual NFT items.
- `contracts/messages.tolk`, `contracts/types.tolk`, and `contracts/errors.tolk`
  define shared messages, storage-facing types, and starter errors.
- `wrappers/NftCollection.gen.tolk` and `wrappers/NftItem.gen.tolk` are the
  wrappers used by tests and scripts.
- `tests/nft-collection.test.tolk` and `tests/nft-item.test.tolk` cover
  collection behavior and item behavior in focused reference suites.
- `.github/workflows/ci.yml` runs build, test, lint, and format checks on
  GitHub Actions.

## Build

```bash
acton build
```

## Test

```bash
acton test
```

## Try It In Emulation

```bash
acton run deploy-emulation
```

## Scripts

Scripts in `scripts/` cover deployment and collection or item management:

- `deployCollection.tolk` — deploys an NFT collection with on-chain metadata and royalty params.
- `deployItem.tolk` — mints a single NFT item into an existing collection.
- `deployBatch.tolk` — batch-mints multiple items in a single transaction.
- `transferItem.tolk` — transfers an NFT item to a new owner.
- `changeAdmin.tolk` — changes the admin address of an existing collection.

Run them with `acton script scripts/<name>.tolk`.

## Customize The Starter

1. Update the contracts under `contracts/` for your NFT metadata, minting, and
   collection policy.
2. Adjust `wrappers/NftCollection.gen.tolk` and `wrappers/NftItem.gen.tolk` to
   match the new ABI, or regenerate them with `acton wrapper NftCollection` and
   `acton wrapper NftItem`.
3. Extend the focused test suites under `tests/` with the scenarios you care about.
4. Update metadata defaults and management flows under `scripts/`.

## Deploy To Testnet

The deployment scripts expect a wallet named `deployer`.

1. Create a local wallet and request testnet TON:

```bash
acton wallet new --name deployer --local --airdrop
```

2. Run `acton run deploy-emulation` and confirm the generated metadata,
   collection address, and royalty params look correct.
3. Broadcast the collection deployment to testnet:

```bash
acton script scripts/deployCollection.tolk --net testnet
```

You can also use the generated script aliases:

```bash
acton run deploy-emulation
acton run deploy-testnet
```

If you need higher Toncenter limits for blockchain queries, copy `.env.example`
to `.env` and put `TONCENTER_MAINNET_API_KEY` or `TONCENTER_TESTNET_API_KEY`
there, depending on the network you use. Acton loads `.env` automatically.

## CI

The generated project includes `.github/workflows/ci.yml`, which runs:

- `acton build`
- `acton test`
- `acton check --output-format github`
- `acton fmt --check`

## Documentation

- Quickstart: https://ton-blockchain.github.io/acton/docs/quickstart
- Testing: https://ton-blockchain.github.io/acton/docs/commands/test
- Scripts and deployment: https://ton-blockchain.github.io/acton/docs/commands/script
- Wrappers: https://ton-blockchain.github.io/acton/docs/commands/wrapper
- Wallets: https://ton-blockchain.github.io/acton/docs/commands/wallet
