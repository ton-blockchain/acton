# Code Hash Source Verification Registry

## Summary

This project provides a verification service for TON contract code hashes.
Developers submit source files and compilation parameters. The backend rebuilds
the contract, compares the resulting code hash with the requested target, and
stores a verified source bundle when the hashes match.

The system verifies a pure `code_hash`, not a specific deployed address. A
deployed address can be described as using verified code only if its current
on-chain code hash matches a code hash present in the source registry.

The registry is off-chain. Git stores the source bundles and manifests, and the
runtime registry layer serves reads from a SQLite index. The index can be
rebuilt from the Git repository by scanning
`{source_repository.storage_root}/{code_hash_prefix}/{code_hash_suffix}/`,
where `code_hash_prefix` is the first two characters of the code hash and
`code_hash_suffix` is the rest. The storage root defaults to `sources`.

The verifier uses TON payments on the configured network to limit automated
spam. A separate SQLite ledger prevents payment replay. The backend rebuilds
this ledger from the payment wallet history after each restart.

## Goals

- Allow developers to publish source code that reproducibly compiles to a known
  TON code hash.
- Keep the public lookup key simple: `code_hash`.
- Store enough compilation metadata to make verification reproducible.
- Keep exactly one current source bundle for each code hash.
- Make the registry rebuildable from Git without relying on process-local state.
- Keep the registry implementation pluggable behind Rust traits.
- Require one payment on the configured network for each new public
  verification attempt.
- Bind each payment to one code hash through the transaction comment.
- Rebuild payment replay state from TON history after a server restart.

## Non-Goals

- The system does not write verification proofs on-chain.
- The system does not prove that a source bundle is the only possible source for
  a code hash.
- The system does not verify contract data, initial state, owner, balance, or
  any address-specific property.
- The system does not guarantee that a deployed address will keep using the same
  code forever. Checkers must read the current code hash from the chain.

## Trust Model

The service follows the practical explorer model used by systems such as
Etherscan and Blockscout:

- The verifier service is trusted to run the compilation pipeline before adding
  a source bundle to the registry.
- Git is the source of record for accepted source bundles.
- The SQLite registry index is derived state and can be rebuilt from Git.
- The configured TON Center v3 provider is trusted to report payment
  transactions, message bodies, and finality correctly.
- Users who need stronger assurance can download a bundle, recompute its
  `source_bundle_hash`, recompile it, and compare the resulting `code_hash`.

This is not a trustless proof system. The product must use wording such as
"verified by this source registry" and avoid claiming external anchoring.

## Architecture

The system has four main parts:

1. Verification backend.
2. Payment verification.
3. Source storage.
4. Verification registry.

### Verification Backend

The backend receives verification requests from developers.

Input:

- Target `code_hash`, or an address whose current code hash can be read from
  TON.
- Payment transaction hash for a normal public verification request.
- Source files.
- Compiler configuration.
- Optional build configuration.

Responsibilities:

- Validate the request shape.
- Canonicalize source file paths and metadata.
- Resolve or validate compiler configuration.
- Compile the submitted sources.
- Compute the resulting code hash.
- Compare the computed hash with the target `code_hash`.
- Reject mismatches without writing registry data.
- Build a deterministic source bundle.
- Store the bundle through the registry layer.
- Return verification status and storage metadata.

### Payment Verification

The verifier backend supports payments on TON mainnet and testnet. The current
`acton verify` client remains testnet-only. Verification records are keyed by
code hash, so the same verified code can be used on any TON network. Address
lookups query both configured TON Center providers. If an address exists on
both networks, the backend returns `409 Conflict` with a `matches` array that
contains the network and code hash for both variants.

Acton validates portable source paths before payment. Uploads accept at most
256 files. Source paths are relative, at most 128 ASCII characters, and contain
only letters, numbers, `/`, `.`, `_`, and `-`. Empty components, traversal,
trailing dots, `.git`, the root `output` directory, repeated source extensions,
and case-insensitive duplicates are rejected.

The ticket always binds a code hash, even when the final `/verify` request also
contains an address. The client computes or resolves the code hash before it
requests the ticket. The backend then uses the address as a consistency check.
If the address changes code before submission, verification fails before the
payment claim.

`POST /api/v1/take_ticket` accepts a code hash. If the code hash is verified,
the endpoint returns the stored bundle metadata. No payment is necessary.

When `server.read_only` is enabled, `/take_ticket` and `/verify` return `503`
for code hashes that are not already registered. Existing bundles and all read
endpoints remain available, and repeated submissions still return
`already_verified`.

For new code, the endpoint returns:

- The payment network and address.
- The minimum amount in nanoGRAM.
- The exact comment `acton-verify:v1:<code_hash>`.

The CLI sends a bounceable internal message with this comment. Then it
waits for the finalized recipient transaction and sends its hash to `/verify`.
After `Payment finalized:`, the CLI displays a testnet Actonscan URL. The URL
contains the finalized transaction hash in lowercase hexadecimal form.

The backend gets the transaction from TON Center v3. It accepts the transaction
only when all these conditions are true:

- The transaction is finalized and is not emulated or aborted.
- The transaction account and incoming destination equal the payment address.
- The incoming message is not bounced.
- The incoming value is not less than the configured minimum.
- The comment equals the ticket comment for the requested code hash.

TON Center sees the configured payment address, wallet-history reads, and each
transaction hash that the backend checks. Operators must treat this metadata as
visible to their provider. A compromised provider can bypass the payment gate,
but it cannot make mismatched source code pass compilation.

The payment ledger uses the transaction hash as its primary key. Ledger states
are `processing`, `retryable`, and `consumed`. Concurrent claims for one hash
return a conflict.

A deterministic result consumes the payment. This includes a source mismatch,
a client error, or a generic internal error after the claim. Only a source
storage failure that the backend explicitly classifies as retryable returns the
payment to `retryable` in the same server process.

One payment permits at most three claims, including claims after an expired
processing lease. The fourth claim returns `payment_used` without another
TON Center request. Each claim has a generation number. A stale worker cannot
finish a newer claim after its lease expires.

Recovery preserves payments already marked `consumed`, changes interrupted
`processing` claims to `retryable`, and imports previously unseen finalized
payments as `retryable`. Payments referenced by source manifests in the
published Git revision are reconciled to `consumed`. This lets a client
resubmit after a backend restart without making a published payment reusable.

For a successful public verification, the backend stores the payment
transaction hash in lowercase hexadecimal form. The source manifest and lookup
API include this hash. The verifier UI links the hash to Actonscan testnet.

At startup, the payment verifier is not ready. It reads every page of incoming
history on the selected payment network up to a captured chain tip. It
preserves `consumed` ledger entries, releases interrupted `processing` claims
as `retryable`, and adds previously unseen funded protocol payments as
`retryable`. It then marks every payment referenced by the published source
manifests as `consumed`. The merge
never deletes existing replay evidence.

The startup scan ignores payments below the configured minimum. These payments
cannot authorize verification. Payments without the protocol comment also
cannot authorize verification.

During recovery, `/healthz` and `/take_ticket` return `503`. The server retries
a failed scan with an exponential delay of up to 30 seconds.

For unverified code, `/verify` also returns `503` before it claims the payment.
An already-verified lookup can still return successfully during recovery.

Recovery keeps a payment claimable when it reached the payment wallet before a
server crash but no completed attempt was recorded. An interrupted `processing`
attempt also becomes claimable again while it remains within the three-attempt
limit.

Another request can verify the code after ticket issuance but before source
submission. A request without a payment transaction returns `already_verified`
immediately. When a payment transaction is supplied, `/verify` claims it and
checks the registry again before compilation. If the code hash is already
verified, compilation is skipped, the normal payment finalization marks the
claim as `consumed`, and the request returns `already_verified`.

The payment ledger is a local SQLite database. The current claim and recovery
protocol supports one write-capable verifier process for each payment wallet.
Horizontal replicas require a shared transactional ledger and coordinated
recovery before they can accept verification requests safely.

Payment tickets are not persisted and do not freeze the quoted configuration.
An address rotation invalidates payments sent to the old address. A minimum
amount increase can invalidate a payment that used an earlier quote. Operators
must stop ticket issuance and drain or discard outstanding quotes before either
configuration change. Startup recovery scans only the currently configured
payment address.

### Source Storage

Source storage persists verified bundles in Git:

```text
<storage_root>/
  <first_two_code_hash_characters>/
    <remaining_code_hash_characters>/
      manifest.json
      files/
        ...
```

Git provides:

- Public file hosting.
- Commit history.
- Human review surface.
- Simple mirroring and backup.
- A rebuildable source of record for registry indexes.

The local Docker volume is only a checkout/cache. The remote Git repository is
the durable storage target after every successful push.

At startup, the verifier removes uncommitted files and Git index entries below
the configured storage root. A completed local commit is retained and pushed
before the registry index is served. Changes outside the storage root still
stop startup instead of being modified automatically.

Git commands have a 60-second deadline and cannot prompt for credentials on
stdin. If a push fails or times out, the completed local commit remains pending
and is pushed again before the registry index can be served. This also handles
the ambiguous case where the remote accepted a push but the client did not
receive the success response. Deployments must provide working noninteractive
Git credentials.

### Execution Limits

Duplicate singleton multipart fields are rejected before payment is claimed.
Repeated `files` fields remain supported.

The compiler deadline covers writing stdin, reading stdout and stderr, and
waiting for process exit. Both output pipes are drained concurrently. Output
is limited to 16 MiB on stdout and 64 KiB on stderr; exceeding either limit
terminates the worker. The default deadline is ten seconds and is configured
with `compiler.timeout_ms`. Concurrent worker processes are limited by
`compiler.max_concurrent_compilations`, which defaults to one. Compilation
can run without a concurrency limit when this value is `-1`. Compilation failures,
including resource limits, consume the current claim under the existing payment
policy.

These limits complement the container's memory and process limits and reverse
proxy rate limits. They do not provide an independent OS sandbox for each
compiler. Account lookups have a 30-second provider deadline.

Verification logs contain operation, target hash, result and elapsed time.
Submitted compiler parameters and complete source payloads are not logged.

### Verification Registry

The registry is a trait-based layer over accepted verification records.

Current implementation:

- `SourceVerificationRegistry` stores accepted bundles through `SourceStorage`.
- It validates stored bundle manifests and file hashes.
- It writes accepted bundles to Git and upserts them into SQLite.
- It reports a `code_hash` as verified when the SQLite index contains its
  source bundle.
- If SQLite is missing or stale, it is rebuilt from Git.

The handler layer depends on the `VerificationRegistry` trait, not directly on
Git. This keeps room for future implementations such as:

- Git scan only.
- SQLite index rebuilt from Git.
- PostgreSQL index with Git as durable source storage.
- Read-only mirror index for public deployments.

## Verification Target

The system verifies only `code_hash`.

This means:

- Different deployed addresses with the same code hash share the same
  verification result.
- Verification remains valid even if the source was submitted for a different
  address, as long as the code hash is identical.
- Address-specific state is out of scope.
- A checker must first read the current code hash of the address they care
  about, then query the registry with that code hash.
- A public address submission must request its ticket with that resolved code
  hash. It can include the address in `/verify` to detect a later code change.

## Source Bundle

A source bundle is the reproducible unit of verification.

It contains:

- All source files required by the build.
- A manifest.
- The payment transaction hash for a public verification.
- Compiler configuration.
- Entrypoint and source metadata.
- Compilation parameters.

The stable identifier is `source_bundle_hash`, computed from canonical metadata
and file hashes. Git commit SHAs are useful audit metadata, but they are not the
bundle identity.

A `code_hash` has exactly one immutable source bundle. Once it is verified,
later submissions return the stored `source_bundle_hash` and do not compile or
replace the original bundle.

Source files must be valid UTF-8. The API returns file content as text and does
not expose a base64 source-content field.

## Submission Flow

1. Acton compiles the local contract and computes its code hash.
2. Acton sends the code hash to `/take_ticket`.
3. If the code hash is verified, Acton stops successfully without payment.
4. For new code, the backend returns a payment quote with its network.
5. Acton validates source paths, gets wallet approval and sends the payment with the exact comment.
6. Acton waits for the finalized recipient transaction.
7. Acton sends the sources and recipient transaction hash to `/verify`.
8. The backend resolves the target code hash.
9. The backend claims the payment transaction in the ledger.
10. The backend validates source paths and build metadata.
11. The backend compiles the sources and compares both code hashes.
12. If the hashes differ, the response is `mismatch` and no bundle is stored.
13. If the hashes match, the registry stores the payment hash and source bundle.
14. The backend consumes the payment unless an allowlisted source storage
    failure occurred and the retry budget remains.
15. The API returns `match`, `source_bundle_hash`, and `storage_revision`.
    Acton checks that both returned code hashes match its local compilation.

After a payment is claimed, the verification runs as a tracked task independent
of the HTTP connection. A client disconnect does not leave the claim half
processed. The API can return success only after source publication, registry
indexing, and the final payment-ledger update have all succeeded.

## Lookup Flow

To check whether an address uses verified code:

1. Read the current code hash of the address from TON.
2. Query the registry by `code_hash`.
3. If a valid source bundle exists, the code hash is verified.
4. Return the source bundle, payment hash, and build metadata.
5. A user can independently recompute file hashes and recompile the bundle.

## API Model

Current public endpoints:

```text
POST /api/v1/take_ticket
POST /api/v1/verify
GET /api/v1/openapi.json
GET /healthz
GET /api/v1/verification/status?code_hash=...
GET /api/v1/verification/status?address=...
GET /api/v1/verification/source?code_hash=...
GET /api/v1/verification/source?address=...
GET /api/v1/last_verified?limit=50&offset=0
GET /api/v1/abi?code_hash=...
GET /api/v1/statistics
GET /api/v1/statistics/history
```

Status responses include:

- `code_hash`
- `verified`
- `status`: `unverified`, `queued`, `compiling`, or `verified`; `queued` means
  that the request is waiting for a compiler concurrency slot

Source responses include:

- `code_hash`
- `verified`
- `bundle` with the verified source; the endpoint returns HTTP 404 when the
  code hash has no verified source bundle

Each source bundle includes `source_bundle_hash`, optional `payment_tx_hash`,
`verified_at`, `storage_revision`, `entrypoint`, a grouped `compiler` object,
optional `source_map`, and source `files`.

`/api/v1/abi?code_hash=...` returns HTTP 404 when the requested contract has no
indexed ABI. The unfiltered `/api/v1/abi` collection still returns an empty list.

`payment_tx_hash` is absent only for an authenticated administrative submission
that sets `verified_at` and skips the public payment flow.

Last verified and ABI requests accept `limit` and `offset`. Last verified
responses include `items` and `total`. ABI responses include `items`. Last
verified items are ordered by recent verification time. ABI items contain the
`code_hash` and parsed `abi` JSON.

The API also provides `/api/v1/statistics` and
`/api/v1/statistics/history`. The OpenAPI document defines the response shapes
and all error status codes.

## Failure Handling

Important cases:

- Payment history recovery fails: readiness stays false and the server retries.
- Payment is missing, invalid, insufficient, or for another code hash: request
  fails before compilation.
- Payment is already used or processing: request fails with a conflict.
- Source directory and file I/O failures are retryable. A failed Git push is
  also retryable. The payment state changes to `retryable`.
- A process restart changes an interrupted `processing` payment to `retryable`;
  payments already marked `consumed` and payments referenced by published
  source manifests remain consumed.
- Invalid storage configuration, repository integrity errors, Git commit
  errors, and cleanup errors consume the payment.
- A payment can enter `processing` at most three times. Later claims return
  `payment_used` without a TON Center request.
- An expired claim cannot finish a newer claim because each claim has a
  generation number.
- Other results after a payment claim, including generic internal failures:
  payment state changes to `consumed`.
- Deterministic verification result: payment state changes to `consumed`.
- Compilation fails: no storage write.
- Hash mismatch: no storage write.
- Git write fails: verification request fails.
- Stored bundle cannot be re-read or validated: verification request fails.
- Backend process state is lost: the registry is rebuilt from Git.
- Payment database state is lost: the ledger is rebuilt from history on the
  selected payment network.
- Git content is unavailable: source lookup is temporarily unavailable.

The backend keeps writes deterministic by using `code_hash` as the storage key
and `source_bundle_hash` as the integrity identifier of the current contents.

## Product Wording

Recommended labels:

- "Verified code hash"
- "Source registry"
- "Source bundle"
- "This address currently uses a verified code hash"
- "This source package compiles to this code hash"

Avoid:

- "Verified address"
- "Verified contract owner"
- "Verified deployment"
- "Verified state"
- "On-chain verification proof"
