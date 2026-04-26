## Agent Guidance

- If the `acton` Codex skill is available in this environment, use it for Acton CLI, Tolk, wrappers, tests, scripts, deployment, and `Acton.toml` tasks.
- If the `acton` skill is not available, continue without it. Do not block on installation and do not assume network access is available.
- Treat the contracts under `contracts/` as the source of truth, especially `JettonMinter.tolk`, `JettonWallet.tolk`, `messages.tolk`, `storage.tolk`, and `errors.tolk`.
- Treat the minter and wallet contracts as a coupled system. Keep storage, message formats, tests, wrappers, and deployment behavior consistent across both sides.
- Keep `tests/`, `wrappers/`, and `scripts/` aligned with contract changes.
- When ABI changes are involved, prefer regenerating wrappers with `acton wrapper JettonMinter` and `acton wrapper JettonWallet` over hand-editing wrapper files.
- Prefer this validation loop when feasible: `acton build`, `acton test`, `acton run deploy-emulation`.
- Before proposing broadcast deployment changes or metadata changes, verify the full deployment script in emulation first.
- When command syntax or flags are unclear, verify them with `acton --help` or `acton <command> --help`.
