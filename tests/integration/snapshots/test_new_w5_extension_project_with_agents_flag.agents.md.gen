## Agent Guidance

- If the `acton` Codex skill is available in this environment, use it for Acton CLI, Tolk, wrappers, tests, scripts, deployment, and `Acton.toml` tasks.
- If the `acton` skill is not available, continue without it. Do not block on installation and do not assume network access is available.
- Treat `contracts/SimpleExtension.tolk`, `contracts/types.tolk`, `contracts/w5-types.tolk`, and `contracts/walletv5/` as the source of truth for extension and Wallet V5 behavior.
- Treat the extension and Wallet V5 contracts as a coupled system. Keep storage, message formats, tests, wrappers, and install/delete scripts consistent across both sides.
- Keep `tests/simple-extension.test.tolk`, `wrappers/SimpleExtension.gen.tolk`, `wrappers/WalletV5.gen.tolk`, `wrappers/utils.tolk`, and `scripts/` aligned with contract changes.
- When the SimpleExtension or WalletV5 ABI changes, prefer regenerating `wrappers/SimpleExtension.gen.tolk` and `wrappers/WalletV5.gen.tolk` with `acton wrapper <Name>` over hand-editing the generated wrapper files.
- Prefer this validation loop when feasible: `acton build`, `acton test`, `acton run deploy-emulation`.
- Before proposing broadcast install/delete changes, verify the full deployment and extension flow in emulation first.
- When command syntax or flags are unclear, verify them with `acton --help` or `acton <command> --help`.
