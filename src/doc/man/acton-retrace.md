# acton-retrace(1)

## NAME

acton-retrace --- Replay an on-chain transaction locally for inspection

## SYNOPSIS

`acton retrace` [_options_] _hash_

## DESCRIPTION

Download a transaction by hash and replay it locally in the TON sandbox.

`acton retrace` is useful for debugging failures, inspecting outgoing actions,
reviewing VM and executor logs, and comparing local replay results with the
original on-chain execution.

When `--contract` is provided, Acton also prepares a source-level retrace for
the named project contract by recompiling it with debug info and source maps.
When `--debug` is added, the prepared replay is exposed as a local Debug
Adapter Protocol (DAP) server for editor integration.

## OPTIONS

### Retrace Options

{{#options}}

{{#option "_hash_" }}
Transaction hash in hex format.
{{/option}}

{{#option "`--net` _network_" }}
Network to retrace from.

Without this flag, Acton tries mainnet first and then falls back to testnet.
{{/option}}

{{#option "`--api-key` _key_" }}
TonCenter API key for blockchain queries.
{{/option}}

{{#option "`-v`, `--verbose`" }}
Show full cell hex in outgoing actions instead of only hashes.
{{/option}}

{{#option "`--logs-dir` _dir_" }}
Write VM and executor logs into the specified directory.
{{/option}}

{{/options}}

### Source-Level Debugging Options

{{#options}}

{{#option "`--contract` _name_" }}
Contract name from `Acton.toml` used to build a source-level trace for the
retraced transaction.
{{/option}}

{{#option "`--debug`" }}
Expose the prepared source-level retrace as a local DAP server.

Requires `--contract`.
{{/option}}

{{#option "`--debug-port` _port_" }}
Debug server port to use with `--debug`.

When omitted, Acton uses `12345`.
{{/option}}

{{/options}}

### Display Options

{{> options-display }}

### Project Options

{{> options-project-resolved }}

## NETWORK AND API BEHAVIOR

- Without `--net`, Acton tries mainnet and then testnet
- `localnet` and `custom:<name>` are not supported by the retrace backend
- If no `--api-key` is provided, Acton throttles requests to reduce rate-limit
  failures

For library fallback lookups, retrace also supports `DTON_API_KEY` from the
environment.

## SOURCE-LEVEL RETRACE

When `--contract` is set:

- the contract must exist in the resolved `Acton.toml`
- the selected source must be a `.tolk` contract
- Acton recompiles the contract for the current invocation with debug info and
  source maps
- the compiled local code hash must match the code hash used by the retraced
  transaction

If you run the command outside the project directory, use `--manifest-path` or
`--project-root` so Acton can resolve the contract configuration and import mappings.

## DEBUG MODE

When `--debug` is used together with `--contract`:

- Acton starts a local DAP server on `127.0.0.1:<port>`
- the normal retrace summary is still printed before the debug server starts
- the command stays attached to that debug session until the client disconnects

This mode is intended for editor integrations that speak the Debug Adapter
Protocol.

`--debug-port` follows the same convention as other Acton debug commands:
without `--debug` it does not start a debug server.

## VS CODE SETUP

Typical setup:

1. Install or load the TON VS Code extension.
2. Make sure VS Code can find your `acton` binary.

   If needed, point the extension at a specific binary with an environment
   variable such as `ACTON_BIN`, or start VS Code from a shell where `acton` is
   already on `PATH`.

3. Open the project workspace that contains the target contract in
   `Acton.toml`.
4. Start a retrace debug session either through the extension command or
   through a manual `launch.json` entry.

For a manual setup, create `.vscode/launch.json` like this:

```json
{
  "version": "0.2.0",
  "configurations": [
    {
      "type": "tolk",
      "request": "launch",
      "name": "Attach to Acton Retrace",
      "debugServer": 4711
    }
  ]
}
```

Then start retrace in a terminal from the project root:

```bash
acton retrace <HASH> --contract <NAME> --debug --debug-port 4711
```

After the DAP server is listening, launch the matching VS Code debug
configuration. Breakpoints should be set in the same `.tolk` sources that
belong to the selected contract from `Acton.toml`.

## OUTPUT

The command prints:

- network selection and state-hash check status
- transaction metadata and fee summary
- compute and action phase summaries
- decoded outgoing actions
- additional full VM and executor logs when `--logs-dir` is used
- source-level debug preparation errors when `--contract` is used
- DAP startup status when `--debug` is used

When `--logs-dir` is set, Acton creates the target directory and writes
`vm.log` plus `executor.log` there.

## EXIT STATUS

- `0`: The transaction was fetched and replayed successfully.
- `1`: The transaction could not be found, network lookup failed, replay data
  was incomplete, local contract validation or compilation failed, the local
  code hash did not match the retraced account code, the DAP server could not
  start, or local log files could not be written.

## EXAMPLES

1. Retrace with automatic network fallback:

   ```bash
   acton retrace 3c1b02a33390e596d83b306eab57b3f7271bc90e2e527ea4cafccfde25139d41
   ```

2. Force mainnet and use an API key:

   ```bash
   acton retrace 3c1b02a33390e596d83b306eab57b3f7271bc90e2e527ea4cafccfde25139d41 --net mainnet --api-key YOUR_API_KEY
   ```

3. Save logs and show full outgoing cell bodies:

   ```bash
   acton retrace 3c1b02a33390e596d83b306eab57b3f7271bc90e2e527ea4cafccfde25139d41 --verbose --logs-dir build/retrace
   ```

4. Prepare a source-level retrace for a project contract:

   ```bash
   acton retrace 3c1b02a33390e596d83b306eab57b3f7271bc90e2e527ea4cafccfde25139d41 --contract JettonMinter
   ```

5. Start a local DAP server for editor debugging:

   ```bash
   acton retrace 3c1b02a33390e596d83b306eab57b3f7271bc90e2e527ea4cafccfde25139d41 --contract JettonMinter --debug --debug-port 4711
   ```

## SEE ALSO

- `acton help disasm`
- [Retrace command guide](https://ton-blockchain.github.io/acton/docs/commands/retrace)
