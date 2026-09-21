# TON block synchronization

The `ton-sync` CLI downloads masterchain and shard blocks through P2P.
It uses `crates/ton-p2p` for downloads and `crates/ton-indexer-p2p` for complete
indexing batches. Actonscan uses the same adapter.
It reads a global config and discovers masterchain peers through DHT over ADNL
UDP. The `bootstrap` command queries their capabilities. The `sync` command
downloads masterchain blocks, their proofs, and all shard blocks needed for complete indexing batches through RLDP2.

Downloads check file hashes, block root hashes, block headers, predecessor links,
and Merkle proof roots. Validator signatures and state transitions are not
verified. Shard blocks are checked against IDs from masterchain commitments and predecessor links.
The client does not construct shard states.

## Run against Localton

Start a Localton network, then use its generated `global.config.json`:

```sh
cargo run --manifest-path apps/ton-sync/Cargo.toml --locked -- bootstrap \
  --global-config /path/to/localton/global.config.json \
  --data-dir /path/to/p2p-data \
  --timeout 30
```

To download blocks continuously, use the same network and data directory:

```sh
cargo run --manifest-path apps/ton-sync/Cargo.toml --locked -- sync \
  --global-config /path/to/localton/global.config.json \
  --data-dir /path/to/p2p-data \
  --timeout 30
```

Add `--to-seqno 100` to stop after the complete batch for that masterchain sequence number. Without this option, the command keeps polling until Ctrl-C. Run it
again with the same data directory to resume.

Available since trunk: `sync` downloads shard blocks in parallel (`--parallel 16`, range 1–128).
It includes every new shard block between consecutive masterchain commitments, with support for shard splits and merges.
Use `--masterchain-only` to download only masterchain blocks and proofs.

The default advertised address is `127.0.0.1:0`, which selects an ephemeral UDP
port for a network on the same host. For remote peers, set `--address` to a
reachable IPv4 address and UDP port. The transport binds the UDP port on all
local IPv4 interfaces; the address tells peers where to send their packets.

Completed commands print a JSON report to stdout. Progress and errors go to
stderr. For `bootstrap`, the timeout covers discovery and the capabilities query
together. For `sync`, it bounds each discovery attempt or network request.
Ctrl-C cancels the command and stops the UDP listener.

The data directory holds `adnl.key`, a persistent 32-byte private identity.
Keep it private. Repeated starts reuse the same identity. The command creates
this file atomically with owner-only access on Unix.

## Start near the network head

Available since trunk.

Use a new data directory and add `--from-latest`:

```sh
cargo run --manifest-path apps/ton-sync/Cargo.toml --locked -- sync \
  --global-config global.config.json \
  --data-dir .mainnet-live \
  --address PUBLIC_IPV4:UDP_PORT \
  --from-latest
```

This option queries LiteServer for a recent masterchain block ID, including both hashes.
It checks the reported network zerostate against the global config.
All block and proof downloads then use P2P. The first indexed batch follows this starting block.
Earlier history is skipped. The selected LiteServer must be current for this start to be near the network head.

If the directory already has `checkpoint.json`, the command resumes that checkpoint without contacting LiteServer.
The option does not move an existing download forward or discard its history.
Without `--from-latest`, startup uses only P2P and the configured initial block.
The option also works with `--masterchain-only`.

The starting ID is trusted metadata from LiteServer. Checking its hashes does not validate consensus or prove that it is the network head.

## Calibrate peers

Available since trunk.

Run calibration through the same network route that synchronization will use:

```sh
cargo run --release --manifest-path apps/ton-sync/Cargo.toml -- benchmark-peers \
  --global-config global.config.json \
  --address PUBLIC_IPV4:UDP_PORT \
  --data-dir .ton-sync-calibration \
  --output peers.json
```

The command collects peers for 20 seconds, then probes every discovered peer.
`--parallel 32` bounds concurrent operations. Each peer receives one warm-up
followed by `--samples 3` measured operations, with `--probe-timeout 3` seconds
per operation. Failed peers do not delay others beyond their deadline.
Masterchain and shard downloads have separate arithmetic means. All peers
receive the same requests, and downloaded blocks are validated before timing
is accepted. The profile contains public descriptors, request statistics, and
the reference block IDs. It contains no private identity or block payloads.

Pass `--peers-file peers.json` to `sync` to import the profile. Runtime updates
stay in the synchronization data directory; the supplied profile is read-only.
Use the same option on `benchmark-peers` to seed another full calibration.
Repeat calibration after changing the network route. These timings measure
complete block operations, not ping or indexer throughput.

## Download storage and resume

By default, a fresh synchronization begins at `validator.init_block`, or at the
zerostate when `init_block` is absent. A nonzero initial block is downloaded
first. The global config supplies this starting block and its expected hashes.
Existing checkpoints keep their original starting block when the config changes.

The default data directory is `.ton-sync`. It contains:

- `masterchain/<seqno>-<root_hash>-<file_hash>.boc`: block bytes
- `masterchain/<seqno>-<root_hash>-<file_hash>.proof.boc`: proof bytes
- `checkpoint.json`: network identity, starting block, and last downloaded masterchain block
- `shards/<workchain>/<shard>/<seqno>-<root_hash>-<file_hash>.boc`: shard block bytes
- `batch-checkpoint.json`: last complete masterchain/shard batch
- `sync.lock`: an exclusive operating-system lock for the download writer

Available since trunk: the first complete batch follows the configured starting block.
The starting block supplies the previous shard frontier. A zerostate start includes all shard predecessors of the first masterchain block.

The command flushes both masterchain BOCs before it updates `checkpoint.json`.
It updates `batch-checkpoint.json` only after it saves every shard BOC for that batch.
A restart reuses saved shard files. A missing shard keeps the batch checkpoint unchanged.
The command checks the last committed files when resuming. A checkpoint for a different
network is rejected. Disk errors stop the command without advancing progress.

The checkpoint and final report record `verification: "hashes_and_links"`.
This identifies download integrity checks; it does not claim consensus validation.

The client retries other discovered peers after failed downloads and refreshes
discovery periodically. An empty next-block response only means that the queried
peer lacks the block. The command keeps waiting; it does not claim to have reached
the network head. Synchronization needs a peer that retains the requested history.

## Network configuration

The reader uses `dht.static_nodes.nodes`, `validator.zero_state`, and the optional
`validator.init_block`.
It derives the masterchain overlay from the zerostate file hash. The
`liteservers` field can be absent or empty for P2P-only starts.
Available since trunk: `--from-latest` requires a responsive LiteServer from that field when no download checkpoint exists.

This initial transport supports Ed25519 DHT descriptors with one UDP IPv4
address, zero address priority, and a standard 64-byte signature. Unsupported
or invalid descriptors are rejected. At least one valid entry point is required.

ADNL UDP and DHT use `everscale-network` with its `dht` feature. TON full-node
queries reuse the schema in `ton-fullnode-master`. The shared library implements an RLDP2
download client with RaptorQ decoding. The dependency's legacy RLDP and overlay
runtime features are not enabled.

RLDP2 queries fit in one 768-byte source symbol. Answers can use multiple parts
of up to 2,000,000 bytes each. Block and proof downloads are each limited to 8 MiB.
The client requests uncompressed BOCs, so it does not need a local shard state
for decompression. It does not serve blocks or process block broadcasts yet.

## Compile

This app has its own Cargo workspace and lockfile, like Localton.
The development profile optimizes RaptorQ and disables its expensive internal
debug checks. Application code retains the normal development settings.

```sh
cargo build --manifest-path apps/ton-sync/Cargo.toml --locked
```
