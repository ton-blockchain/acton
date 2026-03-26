# acton-init(1)

## NAME

acton-init --- Initialize Acton support in the current directory

## SYNOPSIS

`acton init` [_options_]

## DESCRIPTION

Initialize Acton support in the current working directory.

This command is intended for existing repositories or ad-hoc directories where
you want to add `Acton.toml`, standard Acton ignore rules, the bundled
standard library, and symlinks to global wallet and library overlays.

If `Acton.toml` already exists, `acton init` does not replace it. Instead it
patches in default mappings when they are missing.

If `Acton.toml` does not exist, the command scans `.tolk` files in the current
directory tree and auto-registers files that define `onInternalMessage` as
contract entry files.

## DISPLAY OPTIONS

{{> options-display }}

## PROJECT OPTIONS

{{> options-project-pass-through }}

## GENERATED AND PATCHED FILES

`acton init` can create or update:

- `Acton.toml`
- `.gitignore`
- `.acton/`
- local symlinks for `global.wallets.toml` and `global.libraries.toml`

When patching `.gitignore`, Acton adds groups for:

- Acton artifacts such as `.acton/`, `gen/`, `build/`, and `lcov.info`
- local and global wallet/library overlay files
- `.env` and mnemonic files

## CONTRACT DISCOVERY

When generating a new `Acton.toml`, contract discovery:

- walks the current directory recursively
- skips hidden directories and entries such as `node_modules`, `target`, `.git`,
  and `.acton`
- considers `.tolk` files only
- treats files with an `onInternalMessage` function as contract entry files

## STANDARD LIBRARY

`acton init` ensures that the bundled Tolk standard library is installed into
`.acton/tolk-stdlib`.

## EXAMPLES

1. Initialize Acton support in an existing repository:

   ```bash
   acton init
   ```

2. Regenerate default mappings in an existing `Acton.toml`:

   ```bash
   acton init
   ```

## SEE ALSO

- `acton help new`
- `acton help doctor`
- [Project initialization guide](https://ton-blockchain.github.io/acton/docs/project-init)
