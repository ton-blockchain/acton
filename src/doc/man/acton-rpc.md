# acton-rpc(1)

## Name

acton-rpc --- Inspect remote chain and account state

## Synopsis

`acton rpc` [_options_] _command_

## Description

Query blockchain account state through a configured network endpoint.

`acton rpc` is intended for fast inspection workflows when you want to:

- fetch the latest masterchain block number for a network
- inspect the latest masterchain block object returned by TonCenter
- check whether an account is active, frozen, or uninitialized
- inspect balance, last transaction metadata, and state hashes
- render a TonCenter v3 trace as a decoded transaction tree
- match deployed code against a local Acton project by `code_hash`
- decode account storage through local or bundled ABI metadata when a match is
  found

The command works without a project manifest for raw remote inspection.

When Acton can resolve a local project and finds a contract with the same
compiled `code_hash`, it also prints the matched contract name and decodes the
account storage using the local compiler ABI. If no local match exists, Acton
falls back to its bundled ABI catalog.

## Subcommands

### acton rpc info

Show information about a single account.

#### Synopsis

`acton rpc info` [_options_] _address_

#### Options

{{#options command="acton rpc info"}}

{{#option "_address_" }}
Contract address in friendly or raw format.
{{/option}}

{{#option "`--net` _network_" }}
Network to query.

Defaults to `testnet`.

Supported values include `mainnet`, `testnet`, `localnet`, and
`custom:<name>`.
{{/option}}

{{/options}}

#### Output

`acton rpc info` prints:

- remote account metadata such as status, balance, last transaction LT and
  hashes
- code and data hashes when the account has deployed state
- ABI match information when a project contract or bundled catalog entry has
  the same `code_hash`
- decoded storage in a YAML-like view when compiler ABI metadata is available

If no ABI match is found, Acton still prints the raw remote account information
and reports that decoded storage is unavailable.

### acton rpc call

Call a contract get-method through TonCenter.

#### Synopsis

`acton rpc call` [_options_] _address_ _method_ [_args_...]

#### Options

{{#options command="acton rpc call"}}

{{#option "_address_" }}
Contract address in friendly or raw format.
{{/option}}

{{#option "_method_" }}
Get-method name or numeric TVM method id.
{{/option}}

{{#option "_args_" }}
Arguments to pass to the get-method.
{{/option}}

{{#option "`--net` _network_" }}
Network to query.

Defaults to `testnet`.

Supported values include `mainnet`, `testnet`, `localnet`, and
`custom:<name>`.
{{/option}}

{{#option "`--json`" }}
Print machine-readable JSON output.
{{/option}}

{{#option "`--raw`" }}
Print the raw TonCenter stack without ABI decoding.
{{/option}}

{{/options}}

#### ABI Arguments

When Acton finds local or bundled ABI metadata for the remote contract,
get-method arguments are parsed against that ABI.

The _method_ argument can be either an ABI get-method name or a numeric TVM
method id. When the numeric id is present in the ABI, Acton still uses ABI
metadata for arguments and result decoding. When the numeric id is not present
in the ABI, Acton sends the call as a raw get-method request.

- integers use Tolk integer literal syntax such as `42`, `-1`, `0xff`, and
  `0b1010`
- `bool` accepts `true` and `false`
- nullable supported types accept `null`
- `cell`, `slice`, and `bitsN` accept plain BoC hex without `C{}` or `CS{}`
  prefixes
- `any_address` accepts an internal address or the `addr_none` literal
- arrays accept `[item1, item2]`

Without ABI metadata, `acton rpc call` builds a raw stack from CLI arguments.
Raw arguments support Tolk integer literals, `true`, `false`, `null`, internal
addresses, `addr_none`, plain BoC hex as `cell`, and explicit `cell:`, `slice:`,
`builder:`, and `string:` prefixes.

#### Output

When ABI metadata is available and the result stack width matches the
get-method return type, Acton prints the decoded Tolk value. Otherwise it prints
the raw TonCenter stack in a compact field-per-line format.

### acton rpc block

Print the latest masterchain block info returned by TonCenter.

#### Synopsis

`acton rpc block` [_options_]

#### Options

{{#options command="acton rpc block"}}

{{#option "`--net` _network_" }}
Network to query.

Defaults to `testnet`.

Supported values include `mainnet`, `testnet`, `localnet`, and
`custom:<name>`.
{{/option}}

{{/options}}

#### Output

`acton rpc block` prints the full TonCenter `getMasterchainInfo` JSON response
for the selected network.

### acton rpc block-number

Print the latest masterchain block number for a network.

#### Synopsis

`acton rpc block-number` [_options_]

#### Options

{{#options command="acton rpc block-number"}}

{{#option "`--net` _network_" }}
Network to query.

Defaults to `testnet`.

Supported values include `mainnet`, `testnet`, `localnet`, and
`custom:<name>`.
{{/option}}

{{/options}}

#### Output

`acton rpc block-number` prints only the latest masterchain block `seqno` as a
decimal number.

### acton rpc trace

Fetch a TonCenter v3 trace by root transaction hash and render it in a stable
text format.

#### Synopsis

`acton rpc trace` [_options_] _hash_

#### Options

{{#options command="acton rpc trace"}}

{{#option "_hash_" }}
Root transaction hash to query through TonCenter v3 `/traces`.
{{/option}}

{{#option "`--net` _network_" }}
Network to query.

Defaults to `testnet`.

Supported values include `mainnet`, `testnet`, `localnet`, and
`custom:<name>`.
{{/option}}

{{#option "`--summary`" }}
Print only the trace summary.
{{/option}}

{{#option "`--tree`" }}
Print the trace summary and transaction tree. This is the default mode.
{{/option}}

{{#option "`--verbose`" }}
Print the summary, tree, and stable per-transaction fields.
{{/option}}

{{#option "`--show-bodies`" }}
Print decoded message bodies in the transaction tree.
{{/option}}

{{/options}}

#### Output

`acton rpc trace` prints a short summary first:

- query hash
- trace id
- root transaction hash
- whether the trace is complete
- total transaction and message counts

Tree and verbose modes then reuse the same transaction tree formatter as Acton
tests. When current account code matches a local contract or bundled catalog
entry, Acton prints the matched contract name. Add `--show-bodies` to print
decoded inbound message bodies.

## Display Options

{{> options-display }}

## Project Options

{{> options-project-pass-through }}

## Network Resolution

- `mainnet` and `testnet` use the built-in TonCenter endpoints
- `localnet` uses the configured localnet or its default URL
- `custom:<name>` resolves through `[networks.<name>]` in `Acton.toml`

For `custom:<name>`, Acton needs access to the selected project or manifest so
it can read the custom network configuration.

## TonCenter API Keys

Built-in `mainnet`/`testnet` requests read `TONCENTER_MAINNET_API_KEY` or
`TONCENTER_TESTNET_API_KEY`, depending on the selected network.

For `custom:<name>`, Acton reads `<NORMALIZED_NAME>_API_KEY`. Custom network
names are uppercased and non-alphanumeric characters are replaced with `_`, so
`custom:mock` becomes `MOCK_API_KEY`.

Acton loads `.env` automatically, so the simplest setup during project work is
usually to keep these keys there and use shell environment variables only for
one-off overrides or CI.

## ABI Matching

Storage decoding is best-effort and depends on local project context or the
bundled ABI catalog.

Acton attempts to:

1. fetch the remote account code cell
2. compute its `code_hash`
3. compare that hash with locally configured contracts
4. fall back to the bundled ABI catalog when there is no local ABI
5. decode storage with the matched compiler ABI

This means decoding is robust for contracts you control in the current Acton
project, and available for known third-party deployments in the bundled
catalog.

## Exit Status

- `0`: The selected RPC query completed successfully.
- `1`: The address was invalid, the network could not be resolved, the remote
  request failed, or ABI decoding encountered an unrecoverable error.

## Examples

1. Inspect a testnet account quickly:

   ```bash
   acton rpc info EQC...
   ```

2. Inspect a mainnet account with an API key from the environment:

   ```bash
   TONCENTER_MAINNET_API_KEY=your-key acton rpc info EQC... --net mainnet
   ```

3. Inspect a localnet deployment and decode storage with the current project:

   ```bash
   acton rpc info EQC... --net localnet
   ```

4. Call a get-method with ABI-parsed arguments:

   ```bash
   acton rpc call EQC... get_wallet_data --net mainnet
   ```

5. Call a get-method by numeric TVM method id:

   ```bash
   acton rpc call EQC... 85143 --net mainnet
   ```

6. Pass `addr_none` to an `any_address` get-method argument:

   ```bash
   acton rpc call EQC... accepts_any_address addr_none --net localnet
   ```

7. Use a custom network defined in another manifest:

   ```bash
   acton --manifest-path ../incident/Acton.toml rpc info EQC... --net custom:staging
   ```

8. Print the latest mainnet masterchain block JSON:

   ```bash
   acton rpc block --net mainnet
   ```

9. Print the latest mainnet masterchain block number:

   ```bash
   acton rpc block-number --net mainnet
   ```

10. Print a transaction trace from localnet:

   ```bash
   acton rpc trace <tx-hash> --net localnet
   ```

## See Also

- `acton help disasm`
- `acton help retrace`
- `acton help script`
- [Command reference](https://ton-blockchain.github.io/acton/docs/commands/rpc)
