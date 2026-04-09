# Counter App Template

This project was generated from Acton's `counter` template with
TypeScript app scaffold enabled. It includes a counter contract, Tolk tests and
wrappers under `contracts/`, a generated TypeScript wrapper in `wrappers/`, and
a Vite-based React app in `app/`.

## Layout

- `contracts/src` contains the counter contract and shared Tolk types.
- `contracts/tests` contains integration tests.
- `contracts/wrappers` contains Tolk wrappers used by tests and scripts.
- `contracts/scripts` contains deployment scripts.
- `wrappers/Counter.ts` is the generated TypeScript wrapper consumed by the app.
- `app/` contains the React + Vite frontend.
- `package.json`, `tsconfig.json`, and `vite.config.ts` configure the app
  toolchain.
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

## Notes

- `acton build` compiles the contract using `contracts/src/counter.tolk`.
- `npm run build` runs the contract build and the frontend build.
- `npm run test` delegates to `acton test`.
- The app reads blockchain data through Toncenter. Set
  `VITE_TONCENTER_API_KEY` in `.env` if you need higher rate limits.
