# acton-disasm(1)

## NAME

acton-disasm --- Disassemble TVM bytecode into human-readable TASM

## SYNOPSIS

`acton disasm` [_options_] [_boc-file_]

## DESCRIPTION

Disassemble compiled TVM bytecode from a file, a literal BoC string, or a live
contract address.

This command is useful for debugging compiler output, inspecting deployed code,
following library references, and correlating bytecode with Tolk source via
source maps.

## OPTIONS

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

{{#option "`--api-key` _key_" }}
TonCenter API key for blockchain queries.
{{/option}}

{{#option "`--net` _network_" }}
Network to use for `--address`.

If omitted, Acton tries mainnet first and then falls back to testnet.
{{/option}}

{{#option "`--follow-libraries`" }}
If the code references a library, fetch and disassemble the actual library code
instead of showing only the library hash reference.
{{/option}}

{{/options}}

### Display Options

{{> options-display }}

### Project Options

{{> options-project-resolved }}

## BLOCKCHAIN LOOKUPS

When `--address` is used, Acton fetches code from the selected network.

- Without `--net`, Acton tries mainnet first and then testnet
- `--api-key` helps avoid TonCenter rate limits
- `--follow-libraries` resolves library references when possible

## INPUT PRECEDENCE

- `BOC_FILE` and `--string` are mutually exclusive
- `--address` is used only when neither a file nor `--string` is provided
- `--follow-libraries` only replaces the input when the fetched code is a
  single library reference and the library lookup succeeds
- if library lookup fails, Acton warns and disassembles the original code

## SOURCE MAPS

If you compiled a contract with `acton compile --source-map`, you can pass that
source map here to annotate the disassembly with original Tolk locations.

## EXIT STATUS

- `0`: Disassembly completed successfully, including runs with unresolved
  library references that were left as warnings.
- `1`: BoC input was invalid, a blockchain fetch failed, source-map loading
  failed, or the output file could not be written.

## EXAMPLES

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

## SEE ALSO

- `acton help compile`
- [Disassembly command guide](https://ton-blockchain.github.io/acton/docs/commands/disasm)
