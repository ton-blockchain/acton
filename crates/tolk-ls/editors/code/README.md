# Tolk VS Code Extension

Simple extension to support Tolk language using `acton`.

## Installation

1. Build the Acton:
   ```bash
   cargo build
   ```
2. Build the extension:
   ```bash
   cd crates/tolk-ls/editors/code
   npm install
   npm run compile
   ```
3. Link or Install the extension in VS Code.
   - For development, you can open this folder in VS Code and press `F5` to start a new window with the extension loaded.
   - Or use `vsce package` to create a `.vsix` file.

## Configuration

Set the path to the `acton` binary in your settings:
```json
"tolk.serverPath": "/path/to/emulator-rs/target/debug/acton"
```
