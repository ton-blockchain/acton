# Counter Template

This project was generated from Acton's `counter` template. It includes a small
counter contract, generated wrapper helpers, tests, and a ready-to-run
deployment script.

## What Is Included

- `contracts/Counter.tolk` implements the counter contract.
- `contracts/types.tolk` defines storage and message types.
- `wrappers/Counter.gen.tolk` is the generated wrapper used by tests and
  scripts.
- `tests/counter.test.tolk` covers owner checks, increment, decrement, reset,
  underflow, and invalid-message flows.
- `scripts/deploy.tolk` deploys the contract with the deployer wallet as owner,
  then reads the owner and counter value back after deployment.
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
2. Update `contracts/Counter.tolk` with your contract logic.
3. Adjust `wrappers/Counter.gen.tolk` to match the new ABI, or regenerate it
   with `acton wrapper Counter`.
4. Extend `tests/counter.test.tolk` with the scenarios you care about.
5. Update `scripts/deploy.tolk` with the storage and deployment flow you want.

## Deploy To Testnet

The deployment script expects a wallet named `deployer`.

1. Create a local wallet and request testnet TON:

```bash
acton wallet new --name deployer --local --airdrop
```

2. Broadcast the deployment to testnet:

```bash
acton script scripts/deploy.tolk --net testnet
```

The starter script waits for the transaction and then reads the deployed
counter value back from testnet. You do not need a separate `--fork-net` for
that check.

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
