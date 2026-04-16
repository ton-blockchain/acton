## Agent Guidance

- If the `acton` Codex skill is available in this environment, use it for Acton CLI, Tolk, wrappers, tests, scripts, deployment, and `Acton.toml` tasks.
- If the `acton` skill is not available, continue without it. Do not block on installation and do not assume network access is available.
- Treat the contracts under `contracts/` as the source of truth, especially `NftCollection.tolk`, `NftItem.tolk`, `messages.tolk`, `types.tolk`, and `errors.tolk`.
- Treat the collection and item contracts as a coupled system. Keep storage, message formats, tests, wrappers, and deployment behavior consistent across both sides.
- Keep `tests/nft-collection.test.tolk`, `tests/nft-item.test.tolk`, `wrappers/NftCollectionContract.tolk`, `wrappers/NftItemContract.tolk`, and `scripts/` aligned with contract changes.
- When ABI changes are involved, prefer regenerating wrappers with `acton wrapper NftCollection` and `acton wrapper NftItem` over hand-editing wrapper files.
- Prefer this validation loop when feasible: `acton build`, `acton test`
- Before proposing broadcast deployment changes or metadata changes, verify the full deployment script in emulation first.
- When command syntax or flags are unclear, verify them with `acton --help` or `acton <command> --help`.
