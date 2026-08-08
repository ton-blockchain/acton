# TON Zed Extension

Zed extension for TON development with Tolk syntax highlighting and `ton-ls`
(via `acton ls --stdio`).

## What is included

- `languages/tolk/config.toml`
- `languages/tolk/highlights.scm`
- `languages/tolk/indents.scm`
- `languages/tolk/brackets.scm`
- `grammars/tolk.wasm`
- Rust extension shim (`src/lib.rs`) that starts LSP server

Grammar source is local:

- `crates/tree-sitter-tolk` (via `file://` in `extension.toml`)

Bundled wasm is also copied from local grammar:

- `crates/tree-sitter-tolk/tree-sitter-tolk.wasm`

`languages/tolk/highlights.scm` is Zed-adapted and may differ from
`crates/tree-sitter-tolk/queries/highlights.scm`.

## Local usage in Zed

1. Open Zed extensions panel.
2. Install this folder as a dev extension (`apps/zed-ton`).
3. Ensure LSP binary is available:
   - preferred: `acton` in `PATH`
   - fallback: workspace-local `cargo run --bin acton -- ls --stdio`

If you update grammar in `crates/tree-sitter-tolk`:

```bash
cp crates/tree-sitter-tolk/tree-sitter-tolk.wasm apps/zed-ton/grammars/tolk.wasm
```

Then review:

- `extension.toml` `rev` for local grammar repo
- `languages/tolk/highlights.scm` compatibility with new grammar
