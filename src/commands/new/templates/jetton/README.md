# Jetton Template

This project was generated from Acton's `jetton` template. It includes a jetton
minter contract, a jetton wallet contract, wrappers, tests, and a deployment
script that deploys the minter. Separate management scripts cover minting,
transfers, metadata changes, and admin flows.

## What Is Included

- `contracts/JettonMinter.tolk` implements the jetton minter.
- `contracts/JettonWallet.tolk` implements user jetton wallets.
- `contracts/errors.tolk`, `contracts/messages.tolk`,
  `contracts/storage.tolk`, `contracts/fees-management.tolk`,
  `contracts/jetton-utils.tolk`, and `contracts/sharding.tolk` define shared
  messages, storage, fees, sharding helpers, and starter errors.
- `wrappers/JettonMinter.gen.tolk` and `wrappers/JettonWallet.gen.tolk`
  are the wrappers used by tests and scripts.
- `tests/*.test.tolk` covers state init, gas, bounce handling, wallet behavior,
  admin and governance flows, and protocol validation.
- `scripts/deploy.tolk` builds on-chain metadata, deploys the minter, and reads
  total supply back from the network.
- `.github/workflows/contracts.yml` runs build, format, lint, and test checks
  on GitHub Actions.

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

Scripts in `scripts/` cover deployment and jetton management:

- `deploy.tolk` — deploys the jetton minter and prints minter/admin wallet info.
- `mint.tolk` — mints jettons to a recipient.
- `transfer.tolk` — transfers jettons between wallets.
- `info.tolk` — displays minter and wallet info.
- `change-admin.tolk` — proposes a new minter admin.
- `claim-admin.tolk` — claims pending admin status.
- `change-metadata.tolk` — updates jetton metadata.

Run them directly with `acton script scripts/<name>.tolk` or through the
generated aliases such as `acton run jetton-mint` and
`acton run jetton-info`.

## Customize The Starter

1. Update the contracts under `contracts/` for your token policy and business
   rules.
2. Adjust `wrappers/JettonMinter.gen.tolk` and `wrappers/JettonWallet.gen.tolk`
   to match the new ABI, or regenerate them with `acton wrapper JettonMinter`
   and `acton wrapper JettonWallet`.
3. Extend the focused test suites under `tests/` with the scenarios you care about.
4. Update metadata defaults and deployment behavior in `scripts/deploy.tolk`.

## Deploy To Testnet

The scripts prompt for wallets and addresses by default. For non-interactive
use, set `JETTON_DEPLOYER`, `JETTON_ADMIN`, `JETTON_MINTER_ADDRESS`, and other
script-specific variables in a local `.env` copied from `.env.example`, or in
your shell. Metadata and minting scripts also read:

- `JETTON_NAME`
- `JETTON_DESCRIPTION`
- `JETTON_SYMBOL`
- `JETTON_IMAGE`
- `JETTON_DECIMALS`
- `JETTON_MINT_AMOUNT`

1. Create a local wallet and request testnet GRAM:

```bash
acton wallet new --name deployer --local --airdrop
```

2. Optionally copy `.env.example` to `.env` and customize jetton metadata,
   wallet names, addresses, and mint amounts there.
3. Run `acton run deploy-emulation` and confirm the generated metadata, minter
   address, and initial total supply look correct.
4. Broadcast the deployment to testnet:

```bash
acton script scripts/deploy.tolk --net testnet
```

The starter deploy script waits for the deploy transaction, then reads total
supply back from testnet. Use `acton run jetton-mint` after deployment when you
want to mint supply.

You can also use the generated script aliases:

```bash
acton run deploy-emulation
acton run deploy-testnet
```

If you need higher Toncenter limits for blockchain queries, copy `.env.example`
to `.env` and put `TONCENTER_MAINNET_API_KEY` or `TONCENTER_TESTNET_API_KEY`
there, depending on the network you use. Acton loads `.env` automatically.

## CI

The generated project includes `.github/workflows/contracts.yml`, which runs:

- `acton build`
- `acton fmt --check`
- `acton check --output-format github`
- `acton test`

## Documentation

- Quickstart: https://ton-blockchain.github.io/acton/docs/quickstart
- Testing: https://ton-blockchain.github.io/acton/docs/commands/test
- Scripts and deployment: https://ton-blockchain.github.io/acton/docs/commands/script
- Wrappers: https://ton-blockchain.github.io/acton/docs/commands/wrapper
- Wallets: https://ton-blockchain.github.io/acton/docs/commands/wallet
