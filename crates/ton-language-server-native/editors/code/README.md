# TON Extension

Extension for VSCode and VSCode-based editors with comprehensive support for TON Blockchain
languages and technologies including Tolk, FunC, Fift assembly, TL-B, BoC, and Acton.

**[Features] • [Installation] • [Troubleshooting]**

[Features]: #features
[Installation]: #installation
[Troubleshooting]: #troubleshooting

[![Telegram](https://img.shields.io/badge/TON_Community-white?logo=telegram&style=flat)](https://t.me/tondev_eng)
[![Visual Studio Marketplace Downloads](https://img.shields.io/visual-studio-marketplace/d/ton-core.vscode-ton?color=white&labelColor=white&logo=tsnode&logoColor=black)](https://marketplace.visualstudio.com/items?itemName=ton-core.vscode-ton)
[![Open VSX Downloads](https://img.shields.io/open-vsx/dt/ton-core/vscode-ton?color=white&labelColor=white&logo=vscodium&logoColor=black)](https://open-vsx.org/extension/ton-core/vscode-ton)

---

## Features

Tolk support includes:

- Semantic syntax highlighting
- Code completion with auto import, postfix completion, snippets, imports completion
- Go to definition, type definition
- Find all references, workspace symbol search, symbol renaming
- Automatic import updates when renaming and moving files
- Types and documentation on hover
- Inlay hints for types, parameter names, and more
- On-the-fly inspections with quick fixes
- Signature help inside calls
- Build, test, format, lint, and debug projects based on Acton
- Project workflows powered by the installed Acton CLI

FunC support includes:

- Semantic syntax highlighting
- Code completion, imports completion
- Go to definition
- Find all references, workspace symbol search, symbol renaming
- Automatic import updates when renaming and moving files
- Types and documentation on hover
- Inlay hints for method id
- On-the-fly inspections
- Legacy FunC language support for existing contracts

Fift assembly support includes:

- Basic and semantic syntax highlighting
- Go-to definition
- Inlay hints with instruction gas consumption
- Hover documentation for instructions

TL-B support includes:

- Basic and semantic syntax highlighting
- Go-to definition
- Completion for fields, parameters, and types
- Go-to references for types
- Hover documentation for declarations

BoC support includes:

- Automatic BoC disassembly with syntax highlighting
- Automatic updates on BoC changes

## Quick start

The easiest way to get started with TON development is to use VS Code or editors based on it:

1. Install the [Acton CLI](https://github.com/ton-blockchain/acton) and make sure `acton` is
   available in `PATH`
2. Install the TON extension
   [in VS Code](https://marketplace.visualstudio.com/items?itemName=ton-core.vscode-ton)
   or [in VS Code-based editors](https://open-vsx.org/extension/ton-core/vscode-ton)
3. Open an Acton project with an `Acton.toml` file, or create one with the Acton CLI

The extension starts the language server with `acton ls --stdio`. Set `ton.acton.path` when the
executable is not available in `PATH`. Advanced installations can replace the complete argument
list through `ton.languageServer.args`.

[Acton CLI]: https://github.com/ton-blockchain/acton

## Installation

### VS Code / VSCodium / Cursor / Windsurf

1. Get the latest `.vsix` file from [releases](https://github.com/ton-blockchain/acton/releases), from
   [VS Code marketplace](https://marketplace.visualstudio.com/items?itemName=ton-core.vscode-ton),
   or from [Open VSX Registry](https://open-vsx.org/extension/ton-core/vscode-ton)
2. In VS Code:
    - Open the Command Palette (`Ctrl+Shift+P` or `Cmd+Shift+P`)
    - Type "Install from VSIX"
    - Select the downloaded `.vsix` file
    - Reload VS Code

## Troubleshooting

Open the `TON` output channel to inspect language-client and server startup errors. Verify
`ton.acton.path` and `ton.languageServer.args` when the language server cannot start.

# License

MIT
