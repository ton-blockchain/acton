# acton-rpc(1)

## NAME

acton-rpc --- Inspect remote account state and decode contract storage

## SYNOPSIS

`acton rpc` [_options_] _command_

## DESCRIPTION

Query blockchain account state through a configured network endpoint.

`acton rpc` is intended for fast inspection workflows when you want to:

- check whether an account is active, frozen, or uninitialized
- inspect balance, last transaction metadata, and state hashes
- match deployed code against a local Acton project by `code_hash`
- decode account storage through local ABI metadata when a match is found

The command works without a project manifest for raw remote inspection.

When Acton can resolve a local project and finds a contract with the same
compiled `code_hash`, it also prints the matched contract name and decodes the
account storage using the local compiler ABI.

## SUBCOMMANDS

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

{{#option "`--api-key` _key_" }}
TonCenter API key for blockchain queries.
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

## DISPLAY OPTIONS

{{> options-display }}

## PROJECT OPTIONS

{{> options-project-pass-through }}

## NETWORK RESOLUTION

- `mainnet` and `testnet` use the built-in TonCenter endpoints
- `localnet` uses the configured local LiteNode or its default URL
- `custom:<name>` resolves through `[networks.<name>]` in `Acton.toml`

For `custom:<name>`, Acton needs access to the selected project or manifest so
it can read the custom network configuration.

## ABI MATCHING

Storage decoding is best-effort and depends on local project context.

Acton attempts to:

1. fetch the remote account code cell
2. compute its `code_hash`
3. compare that hash with locally configured contracts
4. decode storage with the matched contract's compiler ABI

This means decoding is robust for contracts you control in the current Acton
project, but not guaranteed for arbitrary third-party deployments.

## EXIT STATUS

- `0`: The selected RPC query completed successfully.
- `1`: The address was invalid, the network could not be resolved, the remote
  request failed, or local ABI decoding encountered an unrecoverable error.

## EXAMPLES

1. Inspect a testnet account quickly:

   ```bash
   acton rpc info EQC...
   ```

2. Inspect a mainnet account with an API key:

   ```bash
   acton rpc info EQC... --net mainnet --api-key $TONCENTER_API_KEY
   ```

3. Inspect a localnet deployment and decode storage with the current project:

   ```bash
   acton rpc info EQC... --net localnet
   ```

4. Use a custom network defined in another manifest:

   ```bash
   acton --manifest-path ../incident/Acton.toml rpc info EQC... --net custom:staging
   ```

## SEE ALSO

- `acton help disasm`
- `acton help retrace`
- `acton help script`
- [Command reference](https://ton-blockchain.github.io/acton/docs/commands)
