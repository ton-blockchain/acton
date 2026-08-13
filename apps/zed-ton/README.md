# TON for Zed

This extension adds TON language support to Zed. It uses the Acton language server, `acton ls`.

## Features

- Tolk syntax highlighting and semantic tokens
- Tolk completion, hover information, diagnostics, navigation, references, rename, formatting, and inlay hints
- `Acton.toml` completion, hover information, validation, and path navigation
- Syntax highlighting for TL-B, TASM, and Fift
- Document outlines, bracket matching, indentation, and Vim text objects
- Run buttons for Tolk scripts and tests
- Acton tasks for scripts, tests, checks, and formatting

The extension supports these files:

- Tolk: `.tolk`
- TL-B: `.tlb`
- TASM: `.tasm`
- Fift: `.fif` and `.fift`
- Acton project files: `Acton.toml`

## Install the extension

Install `TON` from the Zed extension page.

For a development build, use these steps:

1. Install the WASI target.

   ```bash
   rustup target add wasm32-wasip2
   ```

2. Open the Zed extension page.
3. Select **Install Dev Extension**.
4. Select the `apps/zed-ton` directory.

## Language server

The extension selects `acton` in this order:

1. The path in the Zed settings
2. An `acton` executable in the worktree `PATH`
3. A managed Acton release for macOS or Linux

The managed installation supports Apple Silicon, macOS x86-64, Linux AArch64, and Linux x86-64.

Acton does not publish a Windows binary or a 32-bit binary. On these platforms, install Acton and set its path.

Use this Zed setting for a custom installation:

```json
{
  "lsp": {
    "ton-ls": {
      "binary": {
        "path": "/absolute/path/to/acton",
        "arguments": ["ls", "--stdio"],
        "env": {
          "RUST_LOG": "info"
        }
      },
      "initialization_options": {},
      "settings": {}
    }
  }
}
```

The `arguments` value replaces the default value. Keep `ls` and `--stdio` unless a compatible server command needs different arguments.

The run buttons and tasks use `acton` from the terminal `PATH`. Add `acton` to that `PATH` before you use them.

## Development

The canonical Tree-sitter queries are in each `crates/tree-sitter-*/queries` directory.

Zed requires query files in the extension directory. The repository stores synchronized copies in `apps/zed-ton/languages` for release packaging.

Do not change a synchronized copy directly. Change the canonical query, and then run this command:

```bash
cargo xtask sync-zed-queries
```

Run the extension checks before a release:

```bash
just check-zed-extension
```

This command checks query syntax and synchronization. It also runs Rust tests, Clippy, and the WASM release build.

If you change a grammar, update the pinned commit in `extension.toml`. Then synchronize the queries and run the extension checks.
