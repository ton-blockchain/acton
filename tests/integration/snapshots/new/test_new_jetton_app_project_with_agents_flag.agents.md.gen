## Agent Guidance

- If the `acton` Codex skill is available in this environment, use it for Acton CLI, Tolk, wrappers, tests, scripts, deployment, and `Acton.toml` tasks.
- If the `acton` skill is not available, continue without it. Do not block on installation and do not assume network access is available.
- Treat the contracts under `contracts/src/` as the source of truth, especially `JettonMinter.tolk`, `JettonWallet.tolk`, `messages.tolk`, `storage.tolk`, and `errors.tolk`.
- Treat the minter and wallet contracts as a coupled system. Keep storage, message formats, tests, wrappers, TypeScript wrappers, scripts, and frontend flows consistent across both sides.
- Treat `wrappers-ts/JettonMinter.gen.ts` and `wrappers-ts/JettonWallet.gen.ts` as generated output. Prefer regenerating them from the contract ABI instead of hand-editing them when the ABI changes.
- Keep `contracts/tests/`, `contracts/wrappers/`, `contracts/scripts/`, `wrappers-ts/`, and the frontend code in `app/` aligned with contract changes.
- Prefer this validation loop when feasible: `acton build`, `acton test`, `npm run typecheck`, `npm run build`.
- Before proposing broadcast deployment changes or metadata changes, verify the contract flow with `acton run deploy-emulation` first.
- When command syntax or flags are unclear, verify them with `acton --help`, `acton <command> --help`, `npm run`, or the existing project config.
