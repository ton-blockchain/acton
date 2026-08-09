# Actonscan backend

The backend indexes canonical TON blocks from LiteServer. It stores TPS
samples, opcode statistics, and the indexer checkpoint in SQLite.

## Run locally

```sh
ACTONSCAN_CONFIG=apps/actonscan-backend/config.toml \
  cargo run --package actonscan-backend
```

The server listens on `127.0.0.1:3008`. It provides these endpoints:

- `GET /healthz`
- `GET /openapi.json`
- `GET /api/v1/stats/tps`
- `GET /api/v1/stats/opcodes`

## Opcode statistics

`GET /api/v1/stats/opcodes` returns all-time opcode statistics. The default
values are `min_messages=2` and `limit=100`. The maximum limit is 1000.

The backend extracts opcodes from internal messages and outgoing external
messages at their source transaction. It does not count internal messages
again at their destination transaction.

Incoming external messages remain in `total_messages`, but the backend does
not extract opcodes from them. Bodies shorter than 32 bits also remain in
`total_messages`, but not in `messages_with_opcode`.

For a bounced message, the backend skips the 32-bit bounce prefix. SQLite
stores one row per opcode and up to two example transaction hashes. Each hash
is a 32-byte BLOB. A singleton opcode has no stored hash.

After a restart, the indexer continues from the stored checkpoint.

## Configuration

The backend reads `config.toml`. Set `ACTONSCAN_CONFIG` to use a different
file. The container uses `docker/config.toml`.

Set `[storage].database_path` to the SQLite database path. If the parent
directory does not exist, the backend creates it.

## Docker data

The Docker image declares `/var/lib/actonscan` as a volume. Without an explicit
mount, Docker creates an anonymous volume.

For deployments, mount a named volume at `/var/lib/actonscan`. When you replace
the container, use the same volume.
