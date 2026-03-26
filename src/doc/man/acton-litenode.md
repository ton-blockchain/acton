# acton-litenode(1)

## NAME

acton-litenode --- Manage Acton's lightweight local TON node

## SYNOPSIS

`acton litenode` [_options_] _command_

## DESCRIPTION

Manage the local development node used for TON-compatible local execution,
forked-state development, and faucet-based local funding.

## SUBCOMMANDS

### acton litenode start

Start the lightweight TON node.

#### Synopsis

`acton litenode start` [_options_]

#### Options

{{#options}}

{{#option "`--port` _port_" }}
LiteNode server port.
{{/option}}

{{#option "`--fork-net` _network_" }}
Remote network to use for forked account resolution.
{{/option}}

{{#option "`--fork-block-number` _seqno_" }}
Historical block sequence number to fork from.
{{/option}}

{{#option "`--accounts` _name_[,_name_...]_" }}
Wallet names to auto-fund and deploy on startup.
{{/option}}

{{#option "`--api-key` _key_" }}
TonCenter API key for remote blockchain queries.
{{/option}}

{{#option "`--db-path` _path_" }}
Path to a SQLite database for persistent node state.
{{/option}}

{{#option "`--rate-limit` _rps_" }}
Maximum `/api` requests per second to simulate provider rate limits.
{{/option}}

{{#option "`--load-state` _path_" }}
Load LiteNode state from a JSON snapshot before startup.
{{/option}}

{{#option "`--dump-state` _path_" }}
Dump LiteNode state to a JSON snapshot on shutdown.
{{/option}}

{{/options}}

`--load-state` and `--db-path` cannot be used together in the same run.

### acton litenode airdrop

Send TON from the local faucet to an address.

#### Synopsis

`acton litenode airdrop` [_options_] _address_

#### Options

{{#options}}

{{#option "_address_" }}
Recipient address.
{{/option}}

{{#option "`-a`, `--amount` _ton_" }}
Amount of TON to request.
{{/option}}

{{#option "`-p`, `--port` _port_" }}
LiteNode server port.
{{/option}}

{{/options}}

## CONFIGURATION

You can store defaults in `Acton.toml`:

```toml
[litenode]
port = 3000
fork-net = "testnet"
fork-block-number = 55000000
accounts = ["deployer", "user"]
rate-limit = 1
```

CLI flags override config values for the current invocation.

## NOTES

- fork mode allows local development against remote chain state
- `--rate-limit` applies to `/api/*` endpoints, not admin endpoints
- `--dump-state` writes a snapshot during graceful shutdown

## DISPLAY OPTIONS

{{> options-display }}

## PROJECT OPTIONS

{{> options-project-resolved }}

## EXAMPLES

1. Start with defaults:

   ```bash
   acton litenode start
   ```

2. Fork from testnet at a historical block:

   ```bash
   acton litenode start --fork-net testnet --fork-block-number 55000000
   ```

3. Load and dump JSON state snapshots:

   ```bash
   acton litenode start --load-state snapshots/localnet.json --dump-state snapshots/localnet.json
   ```

4. Airdrop local funds:

   ```bash
   acton litenode airdrop UQA_ftKIJsHEAE_UgtFOUK15hPzycZooFuUr8duyY9T3kwwM --amount 25
   ```

## SEE ALSO

- `acton help wallet`
- [Local development node guide](https://ton-blockchain.github.io/acton/docs/local-development-node)
