## Agent Guidance

- If the `acton` Codex skill is available in this environment, use it for Acton CLI, Tolk, wrappers, tests, scripts, deployment, and `Acton.toml` tasks.
- If the `acton` skill is not available, continue without it. Do not block on installation and do not assume network access is available.
- Treat `contracts/src/Counter.tolk` and `contracts/src/types.tolk` as the source of truth for contract behavior and ABI shape.
- Treat `wrappers/Counter.ts` as generated output. Prefer regenerating it from the contract ABI instead of hand-editing it when the ABI changes.
- Keep `contracts/tests/`, `contracts/wrappers/Counter.tolk`, `contracts/scripts/deploy.tolk`, `wrappers/Counter.ts`, and the frontend code in `app/` aligned with contract changes.
- Prefer this validation loop when feasible: `acton build`, `acton test`, `npm run typecheck`, `npm run build`.
- Before proposing broadcast deployment changes, verify the contract flow with `acton run deploy-emulation` first.
- When command syntax or flags are unclear, verify them with `acton --help`, `acton <command> --help`, `npm run`, or the existing project config.
