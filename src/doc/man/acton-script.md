# acton-script(1)

## Name

acton-script --- Execute a standalone Tolk script file

## Synopsis

`acton script` [_options_] _path_ [_args_...]

## Description

Execute a standalone Tolk script.

Scripts are useful for experimentation, deployment flows, blockchain queries,
and one-off operational tasks. Unlike tests, scripts use a `main()` entry point
and can send real transactions when `--net` is provided.

## Options

### Script Options

{{#options}}

{{#option "_path_" }}
Path to the script file to execute.
{{/option}}

{{#option "_args_..." }}
Arguments passed through to the script.
{{/option}}

{{#option "`-v`, `--verbose`" }}
Enable executor debug logs at verbosity level `1`.

Currently only level `1` is supported. Pass `-v` or `--verbose` at most once.
Use this for low-level executor output such as `debug.dumpStack()`. For richer
debug output, use `--backtrace full` or `--debug`.
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
When `--net` is set, omitted `--fork-net` defaults to the selected broadcast
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

{{#option "`--net` _network_" }}
Broadcast to the selected network. If omitted, the script runs in emulation
mode. Conflicting `--net` and `--fork-net` values are rejected.
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

## Script Model

A Tolk script defines a `main()` function and runs as an isolated execution.

- state is not preserved between runs
- local execution uses emulator wallets and balances
- `--fork-net` keeps execution local but resolves remote state
- `--net` sends real transactions using configured wallets

Wallet names referenced by the script are resolved from the merged wallet
configuration, with local `wallets.toml` entries overriding
`global.wallets.toml` on name conflicts.

## Argument Forwarding

Arguments after `_path_` are passed through to the script runtime.

If a forwarded argument could be mistaken for an Acton flag, insert `--` before
the script arguments:

```bash
acton script scripts/query.tolk -- --net-like-value
```

## Side Effects

`acton script` always compiles and executes the selected script. With
`--net`, it can send real blockchain transactions; without `--net`, execution
stays local even when `--fork-net` is used.

Executor debug logs are hidden by default. Re-run with `-v` when you need
level-1 executor output such as `debug.dumpStack()`.

## Exit Status

- `0`: The script completed successfully, including successful broadcast flows.
- `1`: Script execution failed, broadcast submission failed, or remote network
  access such as fork-state resolution failed.

## Safe Execution Order

When a script can affect on-chain state, the usual safe sequence is:

1. `acton build`
2. `acton test`
3. `acton script <path>` without `--net`
4. only then `acton script <path> --net testnet`

## Examples

1. Execute locally in the emulator:

   ```bash
   acton script scripts/deploy.tolk
   ```

2. Show executor debug logs from `debug.*` helpers:

   ```bash
   acton script scripts/deploy.tolk -v
   ```

3. Broadcast to testnet:

   ```bash
   acton script scripts/deploy.tolk --net testnet
   ```

4. Query mainnet state without broadcasting:

   ```bash
   acton script query.tolk --fork-net mainnet --api-key YOUR_API_KEY
   ```

5. Broadcast a deploy flow and print explorer links:

   ```bash
   acton script scripts/deploy.tolk --net testnet --explorer tonscan
   ```

## See Also

- `acton help run`
- [Scripting guide](https://ton-blockchain.github.io/acton/docs/scripting)
- [Wallet setup](https://ton-blockchain.github.io/acton/docs/setup-wallets)
