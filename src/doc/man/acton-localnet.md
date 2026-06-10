# acton-localnet(1)

## Name

acton-localnet --- Manage Acton's local TON network

## Synopsis

`acton localnet` [_options_] _command_

## Description

Manage the local development node used for TON-compatible local execution,
forked-state development, and faucet-based local funding.

## Subcommands

### acton localnet start

Start the local TON network.

#### Synopsis

`acton localnet start` [_options_]

#### Options

{{#options command="acton localnet start"}}

{{#option "`--port` _port_" }}
Localnet server port.
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

{{#option "`--db-path` _path_" }}
Path to a SQLite database for persistent node state.
{{/option}}

{{#option "`--rate-limit` _rps_" }}
Maximum `/api` requests per second to simulate provider rate limits.
{{/option}}

{{#option "`--response-delay-ms` _ms_" }}
Delay TonCenter v2/v3 and Emulate API responses.
{{/option}}

{{#option "`--load-state` _path_" }}
Load Localnet state from a JSON snapshot before startup.
{{/option}}

{{#option "`--dump-state` _path_" }}
Dump Localnet state to a JSON snapshot on shutdown.
{{/option}}

{{/options}}

`--load-state` and `--db-path` cannot be used together in the same run.

### acton localnet airdrop

Send GRAM from the local faucet to an address.

#### Synopsis

`acton localnet airdrop` [_options_] _address_

#### Options

{{#options command="acton localnet airdrop"}}

{{#option "_address_" }}
Recipient address.
{{/option}}

{{#option "`-a`, `--amount` _gram_" }}
Amount of GRAM to request.
{{/option}}

{{#option "`-p`, `--port` _port_" }}
Localnet server port.
{{/option}}

{{/options}}

### acton localnet status

Inspect the current localnet status.

#### Synopsis

`acton localnet status` [_options_]

#### Options

{{#options command="acton localnet status"}}

{{#option "`-p`, `--port` _port_" }}
Localnet server port.
{{/option}}

{{#option "`--json`" }}
Print machine-readable JSON.
{{/option}}

{{/options}}

## Configuration

You can store defaults in `Acton.toml`:

```acton-toml title="Acton.toml"
[localnet]
port = 5411
fork-net = "testnet"
fork-block-number = 55000000
accounts = ["deployer", "user"]
rate-limit = 1
response-delay-ms = 300
```

CLI flags override config values for the current invocation.

## TonCenter API Keys

When localnet forks from the built-in `mainnet`/`testnet` backends,
authenticated requests read `TONCENTER_MAINNET_API_KEY` or
`TONCENTER_TESTNET_API_KEY`.

When localnet forks from `custom:<name>`, Acton reads
`<NORMALIZED_NAME>_API_KEY`. Custom network names are uppercased and
non-alphanumeric characters are replaced with `_`, so `custom:mock-remote`
becomes `MOCK_REMOTE_API_KEY`.

Acton loads `.env` automatically, so the simplest setup during project work is
usually to keep these keys there and use shell environment variables only for
one-off overrides or CI.

## Runtime Model

- fork mode allows local development against remote chain state
- `acton localnet start` runs in the foreground until the process is stopped
- Acton starts an HTTP server on `127.0.0.1:<port>` for localnet API, control
  endpoints, and the bundled localnet UI
- the server keeps running until the process is stopped, for example with
  `Ctrl+C`
- the Localnet UI is available on the root path, for example
  `http://127.0.0.1:<port>/`
- the bundled UI is a single-page explorer app, so routes like `/explorer`,
  `/tokens`, `/nfts`, and per-address or per-transaction pages are served from
  the same frontend shell
- the UI reads chain data from `/api/v2` and `/api/v3`, and uses `acton_*`
  control endpoints for local address aliases, registered compiler ABIs,
  status, and snapshot tooling
- when `--port` and `[localnet].port` are both absent, the current runtime
  fallback is `5411`
- `--rate-limit` applies to `/api/*` endpoints, not admin endpoints
- `--response-delay-ms` applies only to `/api/v2`, `/api/v3`, and
  `/api/emulate/v1` endpoints; streaming, control, and UI routes are not
  delayed
- `POST /acton_setNetworkConditions` can change the response delay while the
  server is running, and `GET /acton_nodeInfo` reports the current value
- `--dump-state` writes a snapshot during graceful shutdown

## Control Endpoints

The localnet server exposes `acton_*` control routes for local development
tooling:

- `GET /acton_nodeInfo` returns uptime, latest block seqno, and the active state
  source
- `POST /acton_dumpState` with `{"path":"snapshots/localnet.json"}` writes a
  JSON state snapshot without stopping the server
- `POST /acton_loadState` with `{"path":"snapshots/localnet.json"}` replaces
  the current node state with a JSON state snapshot
- `POST /acton_setShardAccount` with
  `{"address":"<ADDR>","shard_account":"<BASE64_BOC>"}` replaces the selected
  account state with a base64-encoded `ShardAccount` BOC
- `POST /acton_sendInternalMessage` with `{"boc":"<BASE64_BOC>"}` sends a
  base64-encoded internal message BOC through the local internal queue
- `POST /acton_setNetworkConditions` with `{"response_delay_ms":300}` updates
  simulated network latency; use `0` to disable response delay

TonCenter-compatible message endpoints such as `/api/v2/sendBoc` and
`/api/v3/message` accept external-in messages only. Use
`/acton_sendInternalMessage` when local tooling needs to inject a raw internal
message.

Control endpoints are not authenticated and are intended only for local
development. Do not expose the localnet server publicly.

## Persistence

- `--db-path` enables persistent SQLite-backed node state across runs
- `--load-state` initializes state from a JSON snapshot and cannot be combined
  with `--db-path`
- `--dump-state` exports a JSON snapshot on shutdown
- when `--db-path` is not used, node state is ephemeral unless loaded or dumped

## Exit Status

- `0`: The selected localnet subcommand completed successfully. For
  `acton localnet status`, this also includes the selected port not running;
  use `--json` and inspect `running` for automation.
- `1`: Startup failed because port binding, state loading, remote fork
  initialization, faucet handling, or a status/control query failed.

## Display Options

{{> options-display }}

## Project Options

{{> options-project-resolved }}

## Examples

1. Start with defaults:

   ```bash
   acton localnet start
   ```

2. Fork from testnet at a historical block:

   ```bash
   acton localnet start --fork-net testnet --fork-block-number 55000000
   ```

3. Load and dump JSON state snapshots:

   ```bash
   acton localnet start --load-state snapshots/localnet.json --dump-state snapshots/localnet.json
   ```

4. Airdrop local funds:

   ```bash
   acton localnet airdrop UQA_ftKIJsHEAE_UgtFOUK15hPzycZooFuUr8duyY9T3kwwM --amount 25
   ```

5. Start a local integration node with pre-funded accounts:

   ```bash
   acton localnet start --accounts deployer,user --db-path build/localnet.db
   ```

6. Inspect a running localnet:

   ```bash
   acton localnet status --json
   ```

## See Also

- `acton help wallet`
- [Local development node guide](https://ton-blockchain.github.io/acton/docs/localnet/overview)
