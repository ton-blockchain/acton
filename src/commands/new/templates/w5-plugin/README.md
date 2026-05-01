# W5 Plugin Template

This project was generated from Acton's `w5-plugin` template. It includes a
sample Wallet V5 extension contract (`SimpleExtension`) that collects recurring
subscription payments from a W5 wallet, the vendored Wallet V5 contract used
for end-to-end testing, wrappers, focused tests, and ready-to-run scripts that
deploy, install, and remove the extension.

## What Is Included

- `contracts/SimpleExtension.tolk` implements the extension contract.
- `contracts/types.tolk`, `contracts/errors.tolk`, and `contracts/w5-types.tolk`
  define storage, messages, errors, and W5 action types used by the extension.
- `contracts/walletv5/` vendors the upstream Wallet V5 contract and its
  supporting modules so the extension can be tested against a real wallet.
- `wrappers/SimpleExtension.gen.tolk` is the generated wrapper used by tests
  and scripts.
- `wrappers/WalletV5Contract.tolk` is a hand-written wrapper that drives the
  vendored Wallet V5 from tests and scripts.
- `tests/simple-extension.test.tolk` covers install, payment collection,
  payment interval enforcement, and admin-driven cancellation flows.
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

Scripts in `scripts/` cover deployment and extension management:

- `deploy.tolk` — deploys the extension with the deployer wallet as admin.
- `install-extension.tolk` — adds the deployed extension to a real Wallet V5
  via a signed external message.
- `delete-extension.tolk` — removes the extension from the wallet via a signed
  external message.

Run them with `acton script scripts/<name>.tolk` or use the generated aliases:

```bash
acton run deploy-emulation
acton run deploy-testnet
acton run install-extension
acton run delete-extension
```

## Customize The Starter

1. Extend `contracts/types.tolk` with your storage, messages, and errors.
2. Update `contracts/SimpleExtension.tolk` with your extension logic.
3. Adjust `wrappers/SimpleExtension.gen.tolk` to match the new ABI, or
   regenerate it with `acton wrapper SimpleExtension`.
4. Extend `tests/simple-extension.test.tolk` with the scenarios you care about.
5. Update the deploy, install, and delete scripts with the flow you want.

## Deploy To Testnet

The deployment scripts expect a Wallet V5 named `deployer-2`.

1. Create a local Wallet V5 and request testnet TON:

```bash
acton wallet new --name deployer-2 --local --airdrop --version v5r1
```

2. Run `acton run deploy-emulation` and confirm the extension address and
   storage look correct.
3. Broadcast the deployment to testnet:

```bash
acton script scripts/deploy.tolk --net testnet
```

4. Install the extension into the wallet, then remove it when you no longer
   need it:

```bash
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
