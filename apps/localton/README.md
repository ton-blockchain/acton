# localton

localton runs an isolated TON development network on one computer.

One command starts a genesis validator, a local DHT server, and a liteserver. The masterchain produces blocks after the network is ready.

localton stores the network state between runs. `Ctrl-C` or `SIGTERM` stops all managed processes and releases their ports.

## Features

- The first run creates a new TON zerostate and all required keys.
- Later runs continue the same blockchain from the stored state.
- The launcher manages full nodes, validators, elections, stakes, rewards, and wallets.
- The native CLI includes liteserver commands and an optional TON HTTP API V2 service.
- Docker Compose adds TON Center API V3, PostgreSQL, Redis, and an event classifier.
- A new zerostate can include active basechain accounts from another network.
- The repository contains no private keys, network state, or binary archives.

## Choose a run mode

If you need API V2 and indexed API V3, use Docker Compose. The image contains all required TON components.

If you need the node, liteserver, wallets, or API V2, use the native CLI. The native CLI does not start API V3.

## Start with Docker Compose

Docker Compose starts the complete service set. Docker automatically selects the `linux/amd64` or `linux/arm64` image.

Run the stack from this repository:

```bash
docker compose up -d
```

Show the service state:

```bash
docker compose ps
docker compose logs -f localton v3-account-scanner v3-worker v3-api
```

The first run can take several minutes. The node must create the network before the V3 worker can index blocks.

Make sure that API V2 and API V3 return data:

```bash
curl http://127.0.0.1:18002/api/v2/getMasterchainInfo
curl http://127.0.0.1:18003/api/v3/masterchainInfo
curl 'http://127.0.0.1:18003/api/v3/blocks?workchain=-1&limit=8&sort=desc'
```

### Docker services

| Service | Purpose |
| --- | --- |
| `localton` | Runs the local TON network and API V2. |
| `postgres` | Stores the indexed blockchain data. |
| `redis` | Stores data for API V3 and the event classifier. |
| `v3-migrations` | Creates the PostgreSQL schema. |
| `v3-account-scanner` | Indexes every account present in the initial network state. |
| `v3-worker` | Reads the validator database and indexes blocks. |
| `v3-api` | Serves TON Center API V3. |
| `v3-classifier` | Classifies indexed actions. |

Docker volumes store the blockchain, keys, PostgreSQL data, and V3 worker state. The image does not contain generated keys or network state.

### Docker ports

Compose publishes only these ports on the host:

| Host endpoint | Service |
| --- | --- |
| `http://127.0.0.1:18000` | Configuration, global config, and health HTTP API. |
| `http://127.0.0.1:18001` | Administrative HTTP API and faucet. |
| `http://127.0.0.1:18002/api/v2` | TON Center API V2. |
| `http://127.0.0.1:18003/api/v3` | TON Center API V3. |
| `127.0.0.1:18004/tcp` | Primary liteserver. |

PostgreSQL, Redis, and DHT stay inside the Compose network. API V2 and API V3 include browser CORS and PNA headers.

The localton configuration and administrative APIs serve their generated contracts at `/openapi.json`:

```text
http://127.0.0.1:18000/openapi.json
http://127.0.0.1:18001/openapi.json
```

Set different host ports before you start Compose:

```bash
LOCALTON_CONFIG_API_PORT=28000 \
LOCALTON_ADMIN_API_PORT=28001 \
LOCALTON_V2_API_PORT=28002 \
LOCALTON_V3_API_PORT=28003 \
docker compose up -d
```

The liteserver stays on port `18004` because the generated global config contains this endpoint.

Run CLI commands inside the node container:

```bash
docker compose exec localton \
  localton status --state-dir /var/lib/localton
```

Stop the services and keep all data:

```bash
docker compose down
```

CAUTION: If you need the current network, do not run the next command. This command deletes all Compose volumes and local keys.

```bash
docker compose down --volumes
```

## Start the native CLI

The automatic TON installation supports macOS and Linux on `arm64` and `x86_64`. The build requires a stable Rust toolchain.

Install the launcher from this repository:

```bash
cargo install --locked --path .
```

Make sure that `$HOME/.cargo/bin` is in `PATH`. Then start the network:

```bash
localton run
```

The default state directory is `.localton`. Use `--state-dir` to keep multiple independent networks:

```bash
localton run --state-dir ./networks/demo
```

The first run performs these operations:

1. It downloads the official TON `v2026.06` archive for the current platform.
2. It makes sure that the archive has the pinned SHA-256 digest.
3. It creates DHT, validator, console, liteserver, and wallet keys.
4. It creates the zerostate and the configuration for `validator-engine`.
5. It starts the local DHT server and the genesis validator.
6. It waits for two different masterchain sequence numbers.
7. It prints the liteserver endpoint, public key, and global configuration path.

Ready output has this form:

```text
Liteserver endpoint: 127.0.0.1:18004
Liteserver public key: <base64 ed25519 public key>
Global config: /absolute/path/to/.localton/global.config.json
```

Keep the launcher process active while you use the network. Press `Ctrl-C` to stop the network.

The next run uses the same state and continues the same chain. It does not create a new zerostate.

### Use an existing TON installation

Pass a directory that contains the required official TON binaries:

```bash
localton run --ton-bin-dir /path/to/ton
```

You can set the same path through the environment:

```bash
TON_BIN_DIR=/path/to/ton localton run
```

The launcher stores this path in `manifest.json`. Later commands use the stored path.

## Native ports

The native network uses these ports by default:

| Endpoint | Purpose |
| --- | --- |
| `127.0.0.1:4441/tcp` | Console for the genesis validator. |
| `127.0.0.1:4442/udp` | ADNL for the genesis validator. |
| `127.0.0.1:18004/tcp` | Primary liteserver. |
| `127.0.0.1:6302/udp` | Local DHT server. |
| `http://127.0.0.1:18000` | Network manifest, global configuration, and health endpoints. |
| `http://127.0.0.1:18001` | Administrative HTTP API and faucet. |
| `http://127.0.0.1:18002/api/v2` | Optional public API V2 proxy. |
| `http://127.0.0.1:18005/api/v2` | Optional internal API V2 backend. |
| `http://127.0.0.1:18006` | Optional API V2 monitor. |

Additional nodes use separate port ranges from `settings.json`.

## TON HTTP API V2

The Docker image already contains API V2. A native installation requires one separate build.

Install the build task from this repository:

```bash
cargo install --locked --path xtask
```

Build API V2 into the selected state directory:

```bash
xtask build-ton-http-api-v2 --state-dir .localton --jobs 8
```

Then start the network with API V2:

```bash
localton run --ton-http-api
```

Make sure that the API returns the current masterchain block:

```bash
curl http://127.0.0.1:18002/api/v2/getMasterchainInfo
```

The build uses the pinned API V2 source and the same TON release as the node. Build artifacts stay under `.localton/tools`.

The native build requires these development components:

- CMake, Ninja, Clang, `clang-format`, and `pkg-config`.
- Boost, OpenSSL, ICU, libsodium, and `libmicrohttpd`.
- LZ4, fmt, hiredis, jemalloc, and c-ares.

## Network configuration

Create `settings.json` without starting the network:

```bash
localton config init
```

Show the persistent configuration:

```bash
localton config show
```

Make sure that the persistent configuration is valid:

```bash
localton config validate
```

Create a new network with three validators:

```bash
localton run --state-dir .localton-three --validators 3
```

Change the enabled validator count in an existing state directory:

```bash
localton config validators 3 --state-dir .localton
```

The configuration contains seven predefined node slots. The first slot is always the genesis validator.

## Import accounts into the zerostate

`--add-account` adds an active basechain account to a new zerostate. The value is a hex-encoded, single-root `ShardAccount` BoC.

Repeat the option to add more accounts:

```bash
localton run \
  --state-dir .localton-imported \
  --add-account <SHARD_ACCOUNT_HEX> \
  --add-account <ANOTHER_SHARD_ACCOUNT_HEX>
```

The launcher reads the address, TON balance, code, data, and private libraries from each `ShardAccount`.

The import accepts only active accounts from workchain `0`. It rejects duplicate addresses and unsupported account features.

Unsupported features include these items:

- Extra currencies.
- Frozen or uninitialized accounts.
- Anycast addresses and split depth.
- Public basechain libraries.

The launcher resets historical transaction fields and storage statistics for the new chain. A `ShardAccount` does not include external global libraries.

The option applies only to a new state directory. Use a new state directory to change the zerostate account set.

## Status and liteserver commands

Show the stored and live network state:

```bash
localton status
localton status --json
```

Use the built-in liteserver client for common queries:

```bash
localton lite last
localton lite account <ADDRESS>
localton lite run-method <ADDRESS> <METHOD>
localton lite shards
localton lite config 15 17 32 34 36
```

Send an external message BoC:

```bash
localton lite send ./message.boc
```

Run a raw official `lite-client` command:

```bash
localton lite exec last
```

## Wallets and local funds

The CLI supports wallet V1, V2, V3, V4R2, V5R1, and Highload V2.

Create and fund a wallet:

```bash
localton wallet create alice --version v4r2
localton wallet fund alice 100
localton wallet info alice
```

Send TON from a managed wallet:

```bash
localton wallet send \
  --from alice \
  --to <ADDRESS_OR_WALLET_NAME> \
  --amount 1.25 \
  --comment hello
```

List managed wallets:

```bash
localton wallet list
```

The CLI does not print private keys. It stores wallet directories with mode `0700` and private key files with mode `0600`.

### Administrative faucet

The administrative API can fund any valid TON address. The `amount` field contains nanoTON as an integer.

This example sends 100 TON:

```bash
curl --request POST http://127.0.0.1:18001/acton_fundAccount \
  --header 'content-type: application/json' \
  --data '{"address":"<ADDRESS>","amount":100000000000}'
```

The request returns after API V2 finds the transfer transaction. Therefore, the faucet requires API V2.

## Nodes and validators

List the configured nodes:

```bash
localton node list
```

Enable and start another validator:

```bash
localton node add node2
```

Add a full node without validator participation:

```bash
localton node add node2 --fullnode-only
```

Start, stop, and inspect a node:

```bash
localton node start node2
localton node stop node2
localton node stats node2
localton node console node2 getstats
```

Disable a node and keep its state:

```bash
localton node remove node2
```

CAUTION: The next command deletes the generated state for `node2`.

```bash
localton node remove node2 --delete-state
```

Show elections and validator sets:

```bash
localton validator status
```

Submit an election request or recover an unfrozen stake:

```bash
localton validator participate node2
localton validator reap node2
```

Process all enabled validators:

```bash
localton validator participate-all
localton validator reap-all
```

Enabled validators automatically create election keys and submit stakes. The launcher also recovers available stakes and rewards.

## Hardfork configuration

Create a hardfork configuration from the latest block of the genesis node:

```bash
localton hardfork
```

Select another source node or include an external message:

```bash
localton hardfork \
  --node node2 \
  --external-message ./message.boc \
  --output ./hardfork.global.config.json
```

The command prints the output path and the block identifier that anchors the fork.

## HTTP services

The native launcher starts the configuration API and administrative API by default. Both services bind to `127.0.0.1` by default.

The configuration API on port `18000` provides these routes:

- `GET /openapi.json` returns its generated OpenAPI document.
- `GET /` returns the network manifest and service endpoints.
- `GET /localhost.global.config.json` returns the global configuration.
- `GET /config` returns the same global configuration.
- `GET /live` and `GET /healthz` return liveness data.
- `GET /add-validator?participate=true` adds a validator and starts election participation.

The administrative API on port `18001` provides these routes:

- `GET /openapi.json` returns its generated OpenAPI document.
- `GET /v1/status` returns the network state.
- `GET /v1/settings` returns the persistent configuration.
- `GET /v1/wallets` returns public wallet data.
- `GET /v1/processes` returns managed process data.
- `POST /v1/nodes/{name}/start` starts a node.
- `POST /v1/nodes/{name}/stop` stops a node.
- `POST /acton_fundAccount` funds an account from the genesis wallet.

Disable either service for one run:

```bash
localton run --no-config-http
localton run --no-admin-http
```

The API V2 proxy preserves methods, paths, queries, bodies, statuses, and end-to-end headers. It also adds browser CORS and PNA headers.

## State and process lifecycle

localton stores all persistent data under the selected state directory. The important files are:

| Path | Content |
| --- | --- |
| `manifest.json` | Network identity, zerostate hashes, liteserver key, and TON binary path. |
| `settings.json` | Persistent network and service configuration. |
| `runtime.json` | Current process identifiers, service endpoints, and the latest observed block. |
| `global.config.json` | Connection data for the local DHT and liteserver. |
| `genesis/` | Validator database, keys, certificates, and node data. |
| `nodes/` | Data for additional nodes. |
| `wallets/` | Managed wallet keys and deployment messages. |
| `logs/` | Standard output and error logs for managed processes. |
| `tools/` | Native API V2 source and build artifacts. |

The launcher locks the state directory while it runs. A second launcher cannot use the same state directory at the same time.

All long-lived child processes belong to one process registry. Normal exit, signals, startup errors, and child failures use the same cleanup path.

If the first genesis build stops early, the next run removes only incomplete genesis artifacts. It keeps the downloaded TON archive cache.

## Security

All keys are generated inside the state directory during the first run. The repository and container image contain no generated private keys.

The default HTTP bind addresses are local-only. Docker Compose also publishes its ports on `127.0.0.1` only.

The administrative API has no authentication. Do not expose its bind address to an untrusted network.

The local [`.gitignore`](.gitignore) file excludes state directories, keyrings, certificates, wallet keys, binaries, and archives.

## Build the container image

Build the complete image locally:

```bash
docker build -t localton:dev .
LOCALTON_IMAGE=localton:dev docker compose up -d
```

For launcher development from the repository root, reuse the native TON and indexer files from the published image. This command builds the Rust workspace and replaces only the `localton` binary:

```bash
just build-localton-dev-image
ACTON_STUDIO_LOCALTON_IMAGE=localton:dev cargo run -- studio start
```

Set `LOCALTON_BASE_IMAGE` to use another base image. Set `ACTON_STUDIO_LOCALTON_IMAGE` to build and run another local tag. The environment variable applies when Studio creates a new full TON network environment. Existing environments keep the image recorded in their runtime configuration.

GitHub Actions publishes `ghcr.io/ton-blockchain/localton` for `linux/amd64` and `linux/arm64`. The workflow publishes `latest`, branch, tag, semantic-version, and commit tags.

## Development checks

Run all source checks from `apps/localton` before you submit a change:

```bash
just check
```

## Scope

localton creates a development network. It does not connect the local validator set to TON mainnet or testnet.

The project does not include an explorer or contract editor. Use API V2, API V3, or the liteserver from another tool.

## References

- [TON documentation](https://docs.ton.org/)
- [TON source repository](https://github.com/ton-blockchain/ton)
- [TON Center indexer](https://github.com/toncenter/ton-indexer)
- [Container package](https://github.com/orgs/ton-blockchain/packages/container/package/localton)

## License

Localton is licensed under either the MIT License or the Apache License, Version 2.0, at your option. See [NOTICE.md](NOTICE.md) for third-party notices.
