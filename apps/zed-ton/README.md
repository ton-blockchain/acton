# TON Zed Extension

Zed extension for TON development with syntax highlighting and `ton-ls`
(via `acton ls --stdio`).

Supported languages:

- Tolk (`.tolk`)
- TL-B (`.tlb`)
- TASM (`.tasm`)
- Fift (`.fif`, `.fift`)
- Acton project configuration (`Acton.toml`, using Zed's built-in TOML grammar)

## What is included

- language definitions and Git-ignored generated query copies under `languages/`
- Rust extension shim (`src/lib.rs`) that starts LSP server

Grammar sources are located in the Acton repository:

- `crates/tree-sitter-tolk`
- `crates/tree-sitter-tlb`
- `crates/tree-sitter-tasm`
- `crates/tree-sitter-fift`

They are referenced as monorepo subdirectories through the `path` entries in
`extension.toml`. Zed builds the grammar WASM files from these sources.

The canonical Tree-sitter queries live beside each grammar under
`crates/tree-sitter-*/queries/`. Zed requires query files inside the extension,
so `apps/zed-ton/languages/*/*.scm` are generated copies. Do not edit those
copies directly or add them to Git.

Generate the extension copies after checkout and after every canonical query
change:

```bash
cargo xtask sync-zed-queries
```

To verify existing local copies without modifying files:

```bash
cargo xtask sync-zed-queries --check
```

## Local usage in Zed

Install the WASI target used by Zed to build Rust extensions:

```bash
rustup target add wasm32-wasip2
```

1. Run `cargo xtask sync-zed-queries` from the repository root.
2. Open Zed extensions panel.
3. Install this folder as a dev extension (`apps/zed-ton`).
4. Ensure LSP binary is available:
   - preferred: `acton` in `PATH`
   - fallback: workspace-local `cargo run --bin acton -- ls --stdio`

If you update a grammar, review:

- the pinned grammar commit in `extension.toml`
- the corresponding canonical queries under `crates/tree-sitter-*/queries/`
- the regenerated extension copies with `cargo xtask sync-zed-queries`
