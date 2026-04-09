# acton-script(1)

## NAME

acton-script --- Execute a standalone Tolk script file

## SYNOPSIS

`acton script` [_options_] _path_ [_args_...]

## DESCRIPTION

Execute a standalone Tolk script.

Scripts are useful for experimentation, deployment flows, blockchain queries,
and one-off operational tasks. Unlike tests, scripts use a `main()` entry point
and can send real transactions when `--broadcast` is enabled.

## OPTIONS

### Script Options

{{#options}}

{{#option "_path_" }}
Path to the script file to execute.
{{/option}}

{{#option "_args_..." }}
Arguments passed through to the script.
{{/option}}

{{/options}}

### Debugging Options

{{#options}}

{{#option "`--debug`" }}
Enable debug mode.
{{/option}}

{{#option "`--backtrace` _level_" }}
Enable execution backtraces.

Currently supported value: `full`.
{{/option}}

{{#option "`--debug-port` _port_" }}
Debug server port.
{{/option}}

{{/options}}

### Cache Options

{{#options}}

{{#option "`--clear-cache`" }}
Clear the compilation cache before running.
{{/option}}

{{/options}}

### Remote Options

{{#options}}

{{#option "`--fork-net` _network_" }}
Fork blockchain state from a remote network for local execution.
With `--broadcast`, omitted `--fork-net` defaults to the selected broadcast
network.
{{/option}}

{{#option "`--fork-block-number` _seqno_" }}
Historical block sequence number to fork from.
{{/option}}

{{#option "`--api-key` _key_" }}
TonCenter API key for blockchain queries.
{{/option}}

{{/options}}

### Broadcasting Options

{{#options}}

{{#option "`--broadcast`" }}
Send transactions to the selected blockchain network instead of emulating them.
Conflicting `--net` and `--fork-net` values are rejected.
{{/option}}

{{#option "`--net` _network_" }}
Network to use for broadcasting.
{{/option}}

{{#option "`--explorer` _name_" }}
Explorer to use for transaction links.

Possible values: `tonscan`, `toncx`, `dton`, `tonviewer`
{{/option}}

{{/options}}

### Output Options

{{#options}}

{{#option "`--show-bodies`" }}
Show decoded message bodies in printed transaction trees when ABI is known.
{{/option}}

{{/options}}

### Display Options

{{> options-display }}

### Project Options

{{> options-project-resolved }}

## SCRIPT MODEL

A Tolk script defines a `main()` function and runs as an isolated execution.

- state is not preserved between runs
- local execution uses emulator wallets and balances
- `--fork-net` keeps execution local but resolves remote state
- `--broadcast` sends real transactions using configured wallets

Wallet names referenced by the script are resolved from the merged wallet
configuration, with local `wallets.toml` entries overriding
`global.wallets.toml` on name conflicts.

## ARGUMENT FORWARDING

Arguments after `_path_` are passed through to the script runtime.

If a forwarded argument could be mistaken for an Acton flag, insert `--` before
the script arguments:

```bash
acton script scripts/query.tolk -- --net-like-value
```

## SIDE EFFECTS

`acton script` always compiles and executes the selected script. With
`--broadcast`, it can send real blockchain transactions; without
`--broadcast`, execution stays local even when `--fork-net` is used.

## EXIT STATUS

- `0`: The script completed successfully, including successful broadcast flows.
- `1`: Script execution failed, broadcast submission failed, or remote network
  access such as fork-state resolution failed.

## SAFE EXECUTION ORDER

When a script can affect on-chain state, the usual safe sequence is:

1. `acton build`
2. `acton test`
3. `acton script <path>` without `--broadcast`
4. only then `acton script <path> --broadcast`

## EXAMPLES

1. Execute locally in the emulator:

   ```bash
   acton script scripts/deploy.tolk
   ```

2. Broadcast to testnet:

   ```bash
   acton script scripts/deploy.tolk --broadcast --net testnet
   ```

3. Query mainnet state without broadcasting:

   ```bash
   acton script query.tolk --fork-net mainnet --api-key YOUR_API_KEY
   ```

4. Broadcast a deploy flow and print explorer links:

   ```bash
   acton script scripts/deploy.tolk --broadcast --net testnet --explorer tonviewer
   ```

## SEE ALSO

- `acton help run`
- [Scripting guide](https://ton-blockchain.github.io/acton/docs/scripting)
- [Wallet setup](https://ton-blockchain.github.io/acton/docs/setup-wallets)
