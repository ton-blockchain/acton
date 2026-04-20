# acton-library(1)

## Name

acton-library --- Manage on-chain TON libraries

## Synopsis

`acton library` [_options_] _command_

## Description

Publish, fetch, inspect, and top up on-chain TON libraries.

Library metadata can also be stored locally in `libraries.toml` or globally in
`global.libraries.toml` for later inspection and maintenance.

## Lifecycle

A typical library workflow is:

1. `publish` a contract or code cell
2. save metadata locally or globally for later lookup
3. inspect runtime state with `info`
4. `fetch` the code again for inspection or backup
5. `topup` the library account when more storage time is needed

## Subcommands

### acton library publish

Publish a contract or arbitrary code as a library.

#### Synopsis

`acton library publish` [_options_] [_contract-name_]

#### Options

{{#options}}

{{#option "_contract-name_" }}
Contract name to publish.

Use `--code` to publish arbitrary code instead of compiling a project contract.
{{/option}}

{{#option "`--code` _code_" }}
Base64 or hex code to publish instead of compiling a contract.
{{/option}}

{{#option "`--duration` _duration_" }}
Requested publication duration such as `100d` or `1y`.
{{/option}}

{{#option "`--wallet` _wallet_" }}
Wallet to use for the publication transaction.
{{/option}}

{{#option "`--net` _network_" }}
Network to use.

Defaults to `testnet`.
{{/option}}

{{#option "`--amount` _ton_" }}
Explicit TON amount to send.

Overrides duration-based estimation.
{{/option}}

{{#option "`-y`, `--yes`" }}
Skip confirmation prompts.
{{/option}}

{{#option "`--local`" }}
Save library metadata to local `libraries.toml`.
{{/option}}

{{#option "`--global`" }}
Save library metadata to global `global.libraries.toml`.
{{/option}}

{{/options}}

### acton library fetch

Fetch library code from the blockchain by hash.

#### Synopsis

`acton library fetch` [_options_] _hash_

#### Options

{{#options}}

{{#option "_hash_" }}
Library code hash.
{{/option}}

{{#option "`--disasm`" }}
Disassemble the fetched code and print TASM.
{{/option}}

{{#option "`-o`, `--output` _path_" }}
Write fetched output to a file.

If `--disasm` is used, the output is text. Otherwise Acton writes binary BoC
for `.boc` paths and base64 text for other paths.
{{/option}}

{{#option "`--net` _network_" }}
Network to use.

Defaults to `testnet`.
{{/option}}

{{#option "`--json`" }}
Emit JSON output for raw library fetches.

If `--disasm` is also passed, Acton prints disassembly text instead.
{{/option}}

{{/options}}

### acton library info

Show information about a deployed library.

#### Synopsis

`acton library info` [_options_] [_name_]

#### Options

{{#options}}

{{#option "_name_" }}
Library name to inspect.

If omitted, Acton can prompt from the available library metadata.
{{/option}}

{{/options}}

The output includes publication time, last top-up time, current balance, and
estimated remaining storage runway.

### acton library topup

Top up a library account for additional storage time.

#### Synopsis

`acton library topup` [_options_] [_library-name_]

#### Options

{{#options}}

{{#option "_library-name_" }}
Library name to top up.
{{/option}}

{{#option "`--duration` _duration_" }}
Requested additional storage duration such as `100d` or `1y`.
{{/option}}

{{#option "`--amount` _ton_" }}
Explicit TON amount to send.

Overrides duration-based estimation.
{{/option}}

{{#option "`--wallet` _wallet_" }}
Wallet to use for the top-up transaction.
{{/option}}

{{#option "`-y`, `--yes`" }}
Skip confirmation prompts.
{{/option}}

{{/options}}

After a successful top-up, Acton updates `last_topup_timestamp` in the stored
library metadata.

## TonCenter API Keys

Built-in `mainnet`/`testnet` requests read `TONCENTER_MAINNET_API_KEY` or
`TONCENTER_TESTNET_API_KEY`, depending on the selected network.

For `custom:<name>`, Acton reads `<NORMALIZED_NAME>_API_KEY`. Custom network
names are uppercased and non-alphanumeric characters are replaced with `_`, so
`custom:mock-remote` becomes `MOCK_REMOTE_API_KEY`.

Acton loads `.env` automatically, so the simplest setup during project work is
usually to keep these keys there and use shell environment variables only for
one-off overrides or CI.

## Display Options

{{> options-display }}

## Project Options

{{> options-project-resolved }}

## Resolution Rules

- library metadata is merged from `global.libraries.toml` first and then local
  `libraries.toml`
- when both files define the same library name, the local entry wins on lookup
- when saving new metadata into a file that already contains the requested
  library ID, Acton appends `-1`, `-2`, and so on to keep the existing record
- if neither `--local` nor `--global` is passed to `publish`, Acton prompts for
  the destination file

## Amount Estimation

For `publish` and `topup`, `--duration` is used to estimate the required TON
amount from the library size and storage duration.

If `--amount` is passed, it overrides that estimate completely.

## Metadata Files

Saved library metadata typically includes:

- name
- hash
- code
- account
- duration
- network
- publication timestamp
- last top-up timestamp
- code size in bits and cells

## Fetch Output

- without `--output`, fetched code is printed to stdout
- with `--output path.boc`, Acton writes binary BoC
- with `--output` to any other path, Acton writes base64 text
- with `--disasm`, stdout and file output are always disassembly text
- with `--json`, Acton prints JSON to stdout and does not also write `--output`
- if both `--json` and `--disasm` are passed, disassembly text takes
  precedence over JSON on successful runs

## Exit Status

- `0`: The selected library subcommand completed successfully.
- `1`: Contract or wallet resolution failed, chain access failed, metadata
  could not be written, or publish/top-up transaction preparation or submission
  failed.

## Examples

1. Publish a contract as a library:

   ```bash
   acton library publish Math --duration 365d --wallet deployer
   ```

2. Publish arbitrary code:

   ```bash
   acton library publish --code "te6cckEBAQEAAgAAAEysuc0=" --duration 100d
   ```

3. Fetch and disassemble a library:

   ```bash
   acton library fetch <HASH> --disasm
   ```

4. Top up a library for one year:

   ```bash
   acton library topup Math --duration 1y
   ```

5. Fetch raw code into a BoC file:

   ```bash
   acton library fetch <HASH> --output build/math-lib.boc
   ```

## See Also

- [Libraries guide](https://ton-blockchain.github.io/acton/docs/advanced/libraries)
