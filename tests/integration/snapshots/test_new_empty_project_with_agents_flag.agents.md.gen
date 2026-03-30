## Agent Guidance

- If the `acton` Codex skill is available in this environment, use it for Acton CLI, Tolk, wrappers, tests, scripts, deployment, and `Acton.toml` tasks.
- If the `acton` skill is not available, continue without it. Do not block on installation and do not assume network access is available.
- Treat `contracts/types.tolk` and `contracts/contract.tolk` as the source of truth for storage, messages, and ABI-facing behavior.
- Keep `tests/wrappers/Empty.tolk`, `tests/contract.test.tolk`, and `scripts/deploy.tolk` aligned with contract changes.
- When ABI changes are involved, prefer regenerating the wrapper with `acton wrapper empty` over hand-editing the wrapper file.
- Prefer this validation loop when feasible: `acton build`, `acton test`, `acton run deploy-emulation`.
- Before proposing broadcast deployment changes, verify the script in emulation first.
- When command syntax or flags are unclear, verify them with `acton --help` or `acton <command> --help`.
