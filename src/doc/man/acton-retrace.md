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

## OUTPUT

The command prints:

- network selection and state-hash check status
- transaction metadata and fee summary
- compute and action phase summaries
- decoded outgoing actions
- additional full VM and executor logs when `--logs-dir` is used

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
   acton retrace 3c1b02a33390e596d83b306eab57b3f7271bc90e2e527ea4cafccfde25139d41 --verbose --logs-dir .acton/retrace
   ```

## SEE ALSO

- `acton help disasm`
- [Retrace command guide](https://ton-blockchain.github.io/acton/docs/commands/retrace)
