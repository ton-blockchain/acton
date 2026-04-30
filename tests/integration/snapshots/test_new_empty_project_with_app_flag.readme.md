# Empty App Template

This project includes a Vite-based React app for TON dApp development. It can
be generated as part of an Acton project with `acton new --template empty --app`
or as a standalone frontend with `acton init --create-app`.

## Layout

- `app/` contains the React + Vite frontend.
- `package.json`, `tsconfig.json`, and `vite.config.ts` configure the app
  toolchain.
- `package-lock.json` pins the npm dependency tree for reproducible installs.
- When generated with `acton new --template empty --app`, the Acton contract
  sources, scripts, tests, and Tolk wrappers live under `contracts/`.

## Install

```bash
npm ci
```

## Commands

```bash
npm run build
npm run typecheck
npm run fmt:check
npm run dev
```

When this app is generated inside an Acton project, the usual Acton commands are
available from the same directory:

```bash
acton build
acton test
acton check
acton fmt --check
```

## Notes

- The app uses Vite, npm, shadcn-style UI primitives, and Tailwind CSS.
- Copy `.env.example` to a local `.env` for Toncenter keys when an Acton project
  provides it. Both Acton CLI and the Vite app read
  `TONCENTER_MAINNET_API_KEY` and `TONCENTER_TESTNET_API_KEY`; Vite allows the
  `TONCENTER_` prefix via `envPrefix` in `vite.config.ts`.
