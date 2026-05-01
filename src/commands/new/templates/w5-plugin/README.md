# W5 Plugin Template

This project was generated from Acton's `w5-plugin` template. It includes a
sample Wallet V5 extension contract (`SimpleExtension`) that collects recurring
subscription payments from a W5 wallet, the vendored Wallet V5 contract used
during emulation, generated wrapper helpers, tests, and scripts to deploy,
install, and remove the extension.

## What Is Included

- `contracts/SimpleExtension.tolk` implements the extension contract.
- `contracts/types.tolk`, `contracts/errors.tolk`, and `contracts/w5-types.tolk`
  define storage, messages, errors, and W5 action types used by the extension.
- `contracts/walletv5/` vendors the upstream Wallet V5 contract and its
  supporting modules so the extension can be tested end-to-end against a real
  wallet.
- `wrappers/SimpleExtension.gen.tolk` is the generated wrapper used by tests
  and scripts.
- `wrappers/WalletV5Contract.tolk` is a hand-written wrapper that drives the
  vendored Wallet V5 from tests and scripts (signed bodies, action lists,
  helpers).
- `tests/simple-extension.test.tolk` covers install, payment collection,
  payment interval enforcement, and admin-driven cancellation flows.
- `scripts/deploy.tolk` deploys the extension with the deployer wallet as
  admin.
- `scripts/install-extension.tolk` and `scripts/delete-extension.tolk` add or
  remove the deployed extension from a real Wallet V5 via signed external
  messages.
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

1. Extend `contracts/types.tolk` with your storage, messages, and errors.
2. Update `contracts/SimpleExtension.tolk` with your extension logic.
3. Adjust `wrappers/SimpleExtension.gen.tolk` to match the new ABI, or
   regenerate it with `acton wrapper SimpleExtension`.
4. Extend `tests/simple-extension.test.tolk` with the scenarios you care about.
5. Update the deploy/install/delete scripts with the storage and flow you want.

## Deploy To Testnet

The deployment scripts expect a wallet named `deployer-2` (a Wallet V5).

1. Create a local wallet and request testnet TON:

```bash
acton wallet new --name deployer-2 --local --airdrop --version v5r1
```

2. Broadcast the deployment to testnet:

```bash
acton script scripts/deploy.tolk --net testnet
```

3. Install the extension into the wallet (after deploy):

```bash
acton run install-extension
```

4. Remove the extension when you no longer need it:

```bash
acton run delete-extension
```

You can also use the generated script aliases:

```bash
acton run deploy-emulation
acton run deploy-testnet
acton run install-extension
acton run delete-extension
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
