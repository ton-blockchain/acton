# Administrative actions in full Studio environments

Studio's **Admin actions** page edits account balances, code, data and lifecycle
state, replaces a complete `ShardAccount`, and updates configuration parameters.
It is available for managed full TON environments. Account edits use hardforks
accepted by the pinned, unmodified TON validator-engine.

Build an image containing **both** the updated Localton binary and the patched
indexer. For development, reuse the pinned image's native TON and API layers:

```sh
docker build -f apps/localton/Dockerfile --target localton-admin-dev \
  -t acton-localton:admin .
ACTON_LOCALNET_IMAGE=acton-localton:admin acton studio
```

Create a full environment with this image. Existing environments retain their
saved image in `runtime.json`; changing the environment variable does not upgrade
them. The complete Dockerfile runtime target also contains the changes. The
`localton-rust-only` target does not upgrade indexing and cannot be used for these
operations. Studio checks compatibility before stopping the environment.

## Execution and recovery

The acton-localnet service runs the operation independently of its HTTP request.
Studio forwards edits to this service, sharing its mutation lock with CLI operations. It saves
cold recovery archives for every node, suspends all validators, verifies a common
masterchain head, builds and installs the same plan on every node, and checks that
each node applied it. It then restores networking and validator keys and waits for
ordinary blocks and the V3 indexer. The operation blocks conflicting lifecycle,
topology and snapshot changes from Studio and the Acton CLI until it finishes.

On failure, the localnet service restores all node archives and rebuilds the derived index.
Recovery runs before startup after an interrupted localnet service process. A retained
`admin-recovery.json` means recovery still needs to complete; it must not be
removed to bypass a failed restore. Archives live in the environment's
`localton-snapshots` Docker volume under `admin/<operation-id>/<service>/` and are
retained for diagnosis. They are separate from ordinary Studio snapshots and
consume disk space until removed. An interrupted operation is reported as failed
rather than being silently resubmitted.

`POST /api/v1/environments/{id}/admin` returns an operation immediately. Poll the
same URL with `GET` for progress and completion. A request contains a UUID `id`:
reuse it when retrying the **same** request after a lost response. Operation
records persist across localnet service restarts. Different content with a reused ID is
rejected.

```json
{
  "kind": "accounts",
  "id": "312a389c-28fc-4d49-b71f-b843cff3b4fd",
  "edits": [{
    "address": "0:2222222222222222222222222222222222222222222222222222222222222222",
    "type": "balance",
    "balance": "42000000000"
  }]
}
```

The API accepts 1–100 distinct accounts per batch. Balances are decimal nanotons;
the UI converts TON amounts. Requests are limited to 16 MiB of JSON and each
base64 BoC string to 16 MiB. `code`, `data`, and `replace` take a base64 `boc`;
`freeze`, `delete`, and `uninit` need no value. `uninit` can optionally set a new
balance. `replace` requires a complete `ShardAccount` with the matching address.
Public masterchain library registrations and account storage statistics are
updated with the account state.

A configuration request has `kind: "config"`, `id`, signed integer `index`, and
base64 `boc`. It uses the configuration master contract and waits for the consumed
seqno and the active parameter value. It also gets cold recovery archives and
post-change production checks.

## Limits

- Only masterchain and a single unsplit workchain-0 shard are supported. Split or
  merged histories are rejected. Every configured node must be available and
  caught up; nodes managed outside the localnet service must not continue validating.
- Several coordinated stops and starts are required. Cost includes copying node
  databases. This is intended for local development, and can take minutes.
- Stock TON's `getState` refuses seqnos above 1000. Localton reconstructs later
  states by applying authenticated Merkle updates, caches the result, and
  revalidates that cache after restarts or restores. The first edit on a long
  chain needs retained block/state history and can be substantially slower.
- Account edits create state discontinuities, without inventing transactions.
  They do not send internal messages, execute code, create message descriptors,
  or enqueue transaction outputs. `RecordedTransaction` is rejected until all of
  those structures can be maintained together. Existing queued messages remain
  queued and can subsequently change the edited account.
- V3 account state indexing handles changes and deletion even when transaction
  LT does not change. Transaction histories retain actual transactions only.
  Derived token/NFT classifications and metadata have their own caches and
  transaction-based update rules; arbitrary code/data replacement does not
  guarantee immediate refresh of every derived view.
- Hardforks use separate tonlib caches, and snapshot restoration clears the V2
  trusted-head cache. API services need to warm up again after these restarts.
- A valid BoC or configuration TL-B value does not prove contract semantics or
  future validator-election correctness. The operation checks acceptance,
  resumed production and indexing, not all future executions. In particular,
  changing consensus/election parameters or system-contract state can have
  delayed effects.
- Direct Localton CLI mutations and manual Docker operations bypass the localnet
  service mutation lock.
  Avoid them while an administrative operation is active.

## Manual CLI steps

With every node stopped, `localton godmode suspend` saves validator keys and
suppresses automatic election actions. Start the suspended nodes, run `godmode
observe` on each, and feed an account-edit array to `godmode prepare` on genesis.
Stop all nodes, install the resulting plan on each using `godmode install`, start
them and run `godmode verify` on each. Stop them again, run `godmode finish` and
`godmode resume` on each, then start normally. Take cold backups first.

The offline commands reject a running Localton process. Installation validates
BoC hashes, headers, predecessor references and shard proofs before changing
files. An incomplete file commit is rolled back before the next node startup.
A pending identical plan is idempotent; another plan cannot replace its block
source. `finish` requires successful verification. Source listeners use available
loopback ports, including on joined nodes.

## Regression checks

```sh
cargo test -p ton-hardfork -p ton-fullnode-master -p ton-localnet -p acton-localnet -p acton-studio
cargo test --manifest-path apps/localton/Cargo.toml
bun run --cwd packages/studio-ui build
bunx playwright test --config packages/studio-ui/playwright.config.ts
ACTON_LOCALNET_IMAGE=acton-localton:admin cargo test -p acton-localnet \
  administrative_hardfork_and_rollback_on_two_nodes -- --ignored --nocapture
```
