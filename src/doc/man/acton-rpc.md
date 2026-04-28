# acton-rpc(1)

## Name

acton-rpc --- Inspect remote chain and account state

## Synopsis

`acton rpc` [_options_] _command_

## Description

Query blockchain account state through a configured network endpoint.

`acton rpc` is intended for fast inspection workflows when you want to:

- fetch the latest masterchain block number for a network
- check whether an account is active, frozen, or uninitialized
- inspect balance, last transaction metadata, and state hashes
- print a transaction trace from TonCenter v3 as an Acton transaction tree
- match deployed code against a local Acton project by `code_hash`
- decode account storage through local ABI metadata when a match is found

The command works without a project manifest for raw remote inspection.

When Acton can resolve a local project and finds a contract with the same
compiled `code_hash`, it also prints the matched contract name and decodes the
account storage using the local compiler ABI.

## Subcommands

### acton rpc info

Show information about a single account.

#### Synopsis

`acton rpc info` [_options_] _address_

#### Options

{{#options}}

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
- local contract match information when a project contract has the same
  `code_hash`
- decoded storage in a YAML-like view when compiler ABI metadata is available

If no local ABI match is found, Acton still prints the raw remote account
information and reports that decoded storage is unavailable.

### acton rpc latest-block

Print the latest masterchain block number for a network.

#### Synopsis

`acton rpc latest-block` [_options_]

#### Options

{{#options}}

{{#option "`--net` _network_" }}
Network to query.

Defaults to `testnet`.

Supported values include `mainnet`, `testnet`, `localnet`, and
`custom:<name>`.
{{/option}}

{{/options}}

#### Output

`acton rpc latest-block` prints only the latest masterchain block `seqno` as a
decimal number.

### acton rpc trace

Fetch a TonCenter v3 trace by root transaction hash and print it in a stable
Acton text format.

#### Synopsis

`acton rpc trace` [_options_] _hash_

#### Options

{{#options}}

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
tests. When current account code matches a local contract in the project,
Acton prints local contract names through the local ABI. Add `--show-bodies`
to print decoded inbound message bodies.

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

Storage decoding is best-effort and depends on local project context.

Acton attempts to:

1. fetch the remote account code cell
2. compute its `code_hash`
3. compare that hash with locally configured contracts
4. decode storage with the matched contract's compiler ABI

This means decoding is robust for contracts you control in the current Acton
project, but not guaranteed for arbitrary third-party deployments.

## Exit Status

- `0`: The selected RPC query completed successfully.
- `1`: The address was invalid, the network could not be resolved, the remote
  request failed, or local ABI decoding encountered an unrecoverable error.

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

4. Use a custom network defined in another manifest:

   ```bash
   acton --manifest-path ../incident/Acton.toml rpc info EQC... --net custom:staging
   ```

5. Print the latest mainnet masterchain block number:

   ```bash
   acton rpc latest-block --net mainnet
   ```

6. Print a transaction trace from localnet:

   ```bash
   acton rpc trace <tx-hash> --net localnet
   ```

## See Also

- `acton help disasm`
- `acton help retrace`
- `acton help script`
- [Command reference](https://ton-blockchain.github.io/acton/docs/commands/rpc)
