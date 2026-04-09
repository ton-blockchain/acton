# Empty Template

This project was generated from Acton's `empty` template. It gives you a
minimal but fully working contract starter with storage, one message, a wrapper,
tests, and a deployment script you can extend.

## What Is Included

- `contracts/contract.tolk` implements a small ownable contract.
- `contracts/types.tolk` defines storage, messages, and starter error codes.
- `tests/wrappers/Empty.tolk` is the wrapper used by tests and scripts.
- `tests/contract.test.tolk` covers deployment and ownership transfer.
- `scripts/deploy.tolk` deploys the contract with `deployer` as the initial
  owner and reads the owner back after deployment.
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

1. Extend `contracts/types.tolk` with your storage and messages.
2. Update `contracts/contract.tolk` with your contract logic.
3. Adjust `tests/wrappers/Empty.tolk` to match the new ABI, or regenerate it
   with `acton wrapper empty`.
4. Extend `tests/contract.test.tolk` with the scenarios you care about.
5. Update `scripts/deploy.tolk` with the storage and deployment flow you want.

## Deploy To Testnet

1. Create a local wallet named `deployer` and fund it on testnet:

```bash
acton wallet new --name deployer --local --airdrop
```

2. Update the starter contract, wrapper, and deploy script for your use case.
3. Run the deployment script against testnet:

```bash
acton script scripts/deploy.tolk --broadcast --net testnet
```

The starter script waits for the transaction and then reads the deployed owner
back from testnet. You do not need a separate `--fork-net` for that check.

The generated `Acton.toml` also includes shortcut scripts:

```bash
acton run deploy-emulation
acton run deploy-testnet
```

If you hit rate limits while talking to Toncenter, set `TONCENTER_API_KEY` in
`.env`.

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
