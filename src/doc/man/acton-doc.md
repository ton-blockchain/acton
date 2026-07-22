# acton-doc(1)

## Name

acton-doc --- Look up reference documentation and contract ABIs from the CLI

## Synopsis

`acton doc` [_options_] _command_

## Description

Look up reference documentation from the terminal.

`acton doc` provides contract ABI lookup through the `abi` subcommand and TVM
instruction lookup through the `tvm` subcommand.

## Subcommands

### acton doc abi

Print compiler ABI JSON for a local or bundled contract.

#### Synopsis

`acton doc abi` _contract_

#### Options

{{#options command="acton doc abi"}}

{{#option "_contract_" }}
Contract name, local contract id, or bundled catalog name.
{{/option}}

{{/options}}

#### Behavior

`acton doc abi` first looks at local contracts from `Acton.toml`, then falls
back to the bundled ABI catalog.

- local lookup accepts the contract id, display name, or ABI contract name
- bundled catalog lookup accepts the catalog display name or ABI contract name
- output is always formatted compiler ABI JSON

### acton doc tvm

Lookup TVM instructions by exact name or fuzzy search query.

#### Synopsis

`acton doc tvm` [_options_] _instruction_...

#### Options

{{#options command="acton doc tvm"}}

{{#option "_instruction_..." }}
Instruction names or search queries.
{{/option}}

{{#option "`-f`, `--find`" }}
Treat the input as a fuzzy search query instead of an exact instruction name.
{{/option}}

{{#option "`-d`, `--description`" }}
Include descriptions in fuzzy matching.

Requires `--find`.
{{/option}}

{{#option "`--json`" }}
Emit JSON instead of formatted text.
{{/option}}

{{/options}}

#### Behavior

Without `--find`, each argument is treated as an instruction name.

- name matching is case-insensitive
- `-` and `#` are normalized for convenience
- JSON mode always returns an array of instruction objects
- at least one instruction name is required

With `--find`, each argument is treated as a search query.

- fuzzy matching uses instruction names by default
- `--description` also matches instruction descriptions, tags, and operands
- JSON mode returns query/matches objects
- at least one query is required
- queries with no matches return an error instead of an empty result set

## Topics

Currently available documentation namespaces:

- `abi`
- `tvm`

Additional namespaces may be added in future releases.

## Exit Status

- `0`: The requested documentation was printed successfully.
- `1`: The namespace was unknown, arguments were invalid, or rendering failed.

## Display Options

{{> options-display }}

## Project Options

{{> options-project-pass-through }}

## Examples

1. Exact lookup:

   ```bash
   acton doc tvm ADD
   acton doc tvm ADD SUB
   ```

2. JSON output:

   ```bash
   acton doc tvm SENDRAWMSG --json
   ```

3. Fuzzy search:

   ```bash
   acton doc tvm SENRAWMSG --find
   acton doc tvm outcomng --find --description
   ```

4. Compare exact lookup with fuzzy search:

   ```bash
   acton doc tvm SENDRAWMSG
   acton doc tvm SENRAWMSG --find
   ```

5. Print contract ABI:

   ```bash
   acton doc abi WalletV4r2
   acton doc abi counter
   ```

## See Also

- [CLI docs lookup guide](https://ton-blockchain.github.io/acton/docs/commands/doc)
