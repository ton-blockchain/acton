# Acton Language Server VS Code Extension

Local VS Code extension for Tolk, TASM, Fift, TL-B, and `Acton.toml`.
It starts the native language server with `acton ls --stdio` by default.

## Installation

1. Build Acton:
   ```bash
   cargo build
   ```
2. Build the extension:
   ```bash
   cd crates/ton-language-server-native/editors/code
   bun install
   bun run build
   ```
3. Link or Install the extension in VS Code.
   - For development, you can open this folder in VS Code and press `F5` to start a new window with the extension loaded.
   - Or use `vsce package` to create a `.vsix` file.

## Configuration

Set the path to the `acton` binary in your settings:
```json
"acton.languageServer.path": "/path/to/emulator-rs/target/debug/acton"
```

For debugging a manually started TCP server:
```json
"acton.languageServer.port": 9257
```
