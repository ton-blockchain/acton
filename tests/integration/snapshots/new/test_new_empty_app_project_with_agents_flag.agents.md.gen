## Agent Guidance

- If the `acton` Codex skill is available in this environment, use it for Acton CLI, Tolk, wrappers, tests, scripts, deployment, and `Acton.toml` tasks.
- If the `acton` skill is not available, continue without it. Do not block on installation and do not assume network access is available.
- Treat the app under `app/` as the source of truth for frontend behavior.
- When this scaffold is generated with `acton new --template empty --app`, keep `contracts/`, `wrappers-ts/Empty.gen.ts`, and the frontend code in `app/` aligned with contract changes.
- Treat `wrappers-ts/Empty.gen.ts` as generated output. Prefer regenerating it from the contract ABI instead of hand-editing it when the ABI changes.
- Prefer this validation loop when feasible: `npm run typecheck`, `npm run build`, and, for full Acton projects, `acton build` and `acton test`.
- When command syntax or flags are unclear, verify them with `acton --help`, `acton <command> --help`, `npm run`, or the existing project config.
