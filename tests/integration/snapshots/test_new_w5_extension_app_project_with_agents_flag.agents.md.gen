## Agent Guidance

- If the `acton` Codex skill is available in this environment, use it for Acton CLI, Tolk, wrappers, tests, scripts, deployment, and `Acton.toml` tasks.
- If the `acton` skill is not available, continue without it. Do not block on installation and do not assume network access is available.
- Treat `contracts/src/SimpleExtension.tolk`, `contracts/src/types.tolk`, `contracts/src/w5-types.tolk`, and `contracts/src/walletv5/` as the source of truth for extension and Wallet V5 behavior.
- Treat the extension contract, Wallet V5 contract, TypeScript wrappers, and frontend flows as a coupled system.
- Treat `contracts/wrappers/SimpleExtension.gen.tolk`, `wrappers-ts/SimpleExtension.gen.ts`, and `wrappers-ts/WalletV5.gen.ts` as generated output. Prefer regenerating them from the contract ABI instead of hand-editing them when the ABI changes.
- Keep `contracts/tests/`, `contracts/wrappers/`, `contracts/scripts/`, `wrappers-ts/`, and the frontend code in `app/` aligned with contract changes.
- Prefer this validation loop when feasible: `acton build`, `acton test`, `npm run typecheck`, `npm run build`.
- Before proposing broadcast install/delete changes, verify the contract flow with `acton run deploy-emulation` first.
- When command syntax or flags are unclear, verify them with `acton --help`, `acton <command> --help`, `npm run`, or the existing project config.
