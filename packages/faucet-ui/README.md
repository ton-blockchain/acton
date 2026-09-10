# Faucet UI

`@acton/faucet-ui` exports the shared `FaucetPage` used by Actonscan and Studio.
It includes address validation, GitHub authentication, request history, and the
WASM proof-of-work worker. Hosts provide a React Router, `ThemeProvider`,
`ToastProvider`, and a `testnetClient` with `getAddressInformation(address)` returning
`{balance: string}`. `faucetBaseUrl` overrides the API endpoint for every request;
`addressPath` controls the destination of the confirmation link.

The standalone entry renders the same page with its own theme and notifications,
using the same-origin faucet API and linking to Actonscan for account details.
It builds to `dist`; integration with the faucet backend is separate.

From the repository root:

```sh
bun --filter @acton/faucet-ui dev
bun --filter @acton/faucet-ui build
bun --filter @acton/faucet-ui test
```

The development server uses port 3008 and proxies API requests to
`https://faucet.ton.org`, so no local backend or localhost CORS configuration is
required. Set `VITE_BACKEND_PROXY_TARGET=http://127.0.0.1:3000` to use a local backend,
or `VITE_FAUCET_URL` to call a different API directly. The backend's
`GITHUB_FRONTEND_URL` must point to the frontend that receives the OAuth callback
(for example, `http://localhost:3008/` or `https://faucet.ton.org/`).

Run `just build-faucet-pow-wasm` from the repository root to regenerate `src/wasm`.
