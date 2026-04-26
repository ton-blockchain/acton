# Jetton Template

This project was generated from Acton's `jetton` template. It includes a jetton
minter contract, a jetton wallet contract, wrappers, tests, and a deployment
script that deploys the minter and mints the initial supply.

## What Is Included

- `contracts/JettonMinter.tolk` implements the jetton minter.
- `contracts/JettonWallet.tolk` implements user jetton wallets.
- `contracts/errors.tolk` defines starter errors for the template.
- `wrappers/JettonMinter.tolk` and `wrappers/JettonWallet.tolk`
  are the wrappers used by tests and scripts.
- `tests/wallet.test.tolk` covers minting, admin updates, content updates, and
  transfers.
- `scripts/deploy.tolk` builds on-chain metadata, deploys the minter, and mints
  the configured supply, then reads total supply back from the network.
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

## Customize The Starter

1. Update the contracts under `contracts/` for your token policy and business
   rules.
2. Adjust `wrappers/JettonMinter.tolk` and `wrappers/JettonWallet.tolk`
   to match the new ABI, or regenerate them with `acton wrapper JettonMinter`
   and `acton wrapper JettonWallet`.
3. Extend `tests/wallet.test.tolk` with the scenarios you care about.
4. Update metadata defaults and deployment behavior in `scripts/deploy.tolk`.

## Deploy To Testnet

The deploy script expects a wallet named `deployer` and optionally reads these
environment variables from `.env` or your shell:

- `JETTON_NAME`
- `JETTON_DESCRIPTION`
- `JETTON_SYMBOL`
- `JETTON_IMAGE`
- `JETTON_SUPPLY`

1. Create a local wallet and request testnet TON:

```bash
acton wallet new --name deployer --local --airdrop
```

2. Optionally customize jetton metadata and supply in `.env`.
3. Run `acton run deploy-emulation` and confirm the generated metadata, minter
   address, and minted supply look correct.
4. Broadcast the deployment to testnet:

```bash
acton script scripts/deploy.tolk --net testnet
```

The starter script waits for deploy and mint transactions, then reads total
supply back from testnet. You do not need a separate `--fork-net` for that
verification step.

You can also use the generated script aliases:

```bash
acton run deploy-emulation
acton run deploy-testnet
```

If you need higher Toncenter limits for blockchain queries, put
`TONCENTER_MAINNET_API_KEY` or `TONCENTER_TESTNET_API_KEY` into the generated
`.env` file, depending on the network you use. Acton loads that file
automatically, so it is usually the easiest place to keep these keys.

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
