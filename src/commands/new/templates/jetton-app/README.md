# Jetton App Template

This project was generated from Acton's `jetton` template with
TypeScript app scaffold enabled. It includes a jetton minter contract,
a jetton wallet contract, Tolk tests and wrappers under `contracts/`,
generated TypeScript wrappers in `wrappers-ts/`, and a Vite-based React
app in `app/`.

## Layout

- `contracts/src` contains the jetton minter and wallet contracts with shared types, messages, and errors.
- `contracts/tests` contains integration tests covering minting, transfers, burns, admin, gas, bounces, and protocol validation.
- `contracts/wrappers` contains Tolk wrappers used by tests and scripts.
- `contracts/scripts` contains deployment and management scripts.
- `wrappers-ts/` contains generated TypeScript wrappers consumed by the app.
- `app/` contains the React + Vite frontend with TON Connect wallet integration.
- `package.json`, `tsconfig.json`, and `vite.config.ts` configure the app toolchain.
- `package-lock.json` pins the npm dependency tree for reproducible installs.

## Install

```bash
npm ci
```

## Commands

```bash
acton build
acton test
npm run build
npm run typecheck
npm run fmt:check
npm run dev
```

## Scripts

- `acton script contracts/scripts/deploy.tolk` — deploys the jetton minter and mints the initial supply.
- `acton script contracts/scripts/mint.tolk` — mints jettons to a recipient.
- `acton script contracts/scripts/transfer.tolk` — transfers jettons between wallets.
- `acton script contracts/scripts/info.tolk` — displays minter and wallet info.
- `acton script contracts/scripts/change-admin.tolk` — changes the minter admin.
- `acton script contracts/scripts/change-metadata.tolk` — updates jetton metadata.
- `acton script contracts/scripts/claim-admin.tolk` — claims pending admin status.

## Notes

- `acton build` compiles the contracts using `contracts/src/JettonMinter.tolk` and `contracts/src/JettonWallet.tolk`.
- `npm run build` runs the contract build and the frontend build.
- `npm run test` delegates to `acton test`.
- The app reads blockchain data through Toncenter. Set
  `VITE_TONCENTER_API_KEY` in `.env` if you need higher rate limits. For Acton
  CLI flows against testnet/mainnet, the generated `.env` is also the easiest
  place to keep `TONCENTER_TESTNET_API_KEY` and `TONCENTER_MAINNET_API_KEY`.
