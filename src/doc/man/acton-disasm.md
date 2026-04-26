# acton-disasm(1)

## Name

acton-disasm --- Disassemble TVM bytecode into human-readable TASM

## Synopsis

`acton disasm` [_options_] [_boc-file_]

## Description

Disassemble compiled TVM bytecode from a file, a literal BoC string, or a live
contract address.

This command is useful for debugging compiler output, inspecting deployed code,
following library references, and correlating bytecode with Tolk source via
source maps.

## Options

### Disassembly Options

{{#options}}

{{#option "_boc-file_" }}
Binary, hex, or base64 BoC file to disassemble.

Use `--string` to pass the code directly instead of reading a file.
{{/option}}

{{#option "`-s`, `--string` _string_" }}
BoC value in hex or base64 format.
{{/option}}

{{#option "`-o`, `--output` _path_" }}
Write disassembled output to a file instead of standard output.

If the parent directory does not exist, Acton creates it automatically.
Existing files are overwritten.
{{/option}}

{{#option "`--show-hashes`" }}
Show cell hashes alongside code blocks.
{{/option}}

{{#option "`--show-offsets`" }}
Show instruction bit offsets in the left column.
{{/option}}

{{#option "`--source-map` _path_" }}
Use a source map to show original Tolk source locations together with
disassembly output.
{{/option}}

{{#option "`--address` _address_" }}
Fetch contract code from the blockchain by address and disassemble it.
{{/option}}

{{#option "`--net` _network_" }}
Network to use for `--address` and library lookups.

Supported values: `mainnet`, `testnet`, `localnet`, `custom:<name>`.

`localnet` and `custom:<name>` resolve through the project network
configuration in `Acton.toml`.

If omitted, Acton tries mainnet first and then falls back to testnet.
{{/option}}

{{#option "`--follow-libraries`" }}
If the input disassembles to exactly one top-level exotic library-reference
cell, fetch and disassemble the actual library code instead of showing only the
library hash reference.

This is not a general recursive library-follow mode.
{{/option}}

{{/options}}

### Display Options

{{> options-display }}

### Project Options

{{> options-project-resolved }}

## Blockchain Lookups

When `--address` is used, Acton fetches code from the selected network.

- `--net` accepts `mainnet`, `testnet`, `localnet`, and `custom:<name>`
- `localnet` and `custom:<name>` use URLs from `Acton.toml`
- Without `--net`, Acton tries mainnet first and then testnet
- with `--address`, `--follow-libraries` reuses the network that returned the
  contract code

When `--follow-libraries` is used with a local file or `--string`, Acton uses
the explicit `--net` if provided; otherwise library fetches default to
testnet.

## TonCenter API Keys

Built-in `mainnet`/`testnet` requests read `TONCENTER_MAINNET_API_KEY` or
`TONCENTER_TESTNET_API_KEY`, depending on the selected network.

For `custom:<name>`, Acton reads `<NORMALIZED_NAME>_API_KEY`. Custom network
names are uppercased and non-alphanumeric characters are replaced with `_`, so
`custom:mock-remote` becomes `MOCK_REMOTE_API_KEY`.

Acton loads `.env` automatically, so the simplest setup during project work is
usually to keep these keys there and use shell environment variables only for
one-off overrides or CI.

## Input Precedence

- `BOC_FILE` and `--string` are mutually exclusive
- `--address` is used only when neither a file nor `--string` is provided
- `--follow-libraries` only replaces the input when the initial disassembly is
  exactly one top-level exotic library-reference cell and the lookup succeeds
- ordinary code, mixed instruction streams, or nested library references are a
  no-op for `--follow-libraries`
- if library lookup fails, Acton warns and disassembles the original code
  instead; this still exits successfully

## Source Maps

If you compiled a contract with `acton compile --source-map`, you can pass that
source map JSON here to annotate the disassembly with original Tolk locations.

`--source-map` only affects annotations in the output. It does not change which
BoC is disassembled.

## Exit Status

- `0`: Disassembly completed successfully, including runs with unresolved
  library references that were left as warnings.
- `1`: BoC input was invalid, a blockchain fetch failed, source-map loading
  failed, or the output file could not be written.

## Examples

1. Disassemble a local BoC file:

   ```bash
   acton disasm contract.boc
   ```

2. Disassemble a literal BoC string:

   ```bash
   acton disasm --string "b5ee9c72010104...0840f01c700f2f4"
   ```

3. Disassemble code from a blockchain address:

   ```bash
   acton disasm --address UQA_ftKIJsHEAE_UgtFOUK15hPzycZooFuUr8duyY9T3kwwM --net testnet
   ```

4. Show hashes, offsets, and source locations:

   ```bash
   acton disasm contract.boc --show-hashes --show-offsets --source-map contract.json
   ```

5. Inspect deployed code together with hashes:

   ```bash
   acton disasm --address UQA_ftKIJsHEAE_UgtFOUK15hPzycZooFuUr8duyY9T3kwwM --show-hashes --net testnet
   ```

6. Write disassembly to a nested output path:

   ```bash
   acton disasm contract.boc --output build/disasm/contract.tasm
   ```

7. Resolve a top-level library-reference cell using a configured custom network:

   ```bash
   acton disasm library-ref.boc --follow-libraries --net custom:staging
   ```

## See Also

- `acton help compile`
- [Disassembly command guide](https://ton-blockchain.github.io/acton/docs/commands/disasm)
