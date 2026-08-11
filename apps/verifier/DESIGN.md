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
`{source_repository.storage_root}/{code_hash}/`. The storage root defaults to
`sources`.

The verifier uses TON testnet payments to limit automated spam. A separate
SQLite ledger prevents payment replay. The backend rebuilds this ledger from
the payment wallet history after each restart.

## Goals

- Allow developers to publish source code that reproducibly compiles to a known
  TON code hash.
- Keep the public lookup key simple: `code_hash`.
- Store enough compilation metadata to make verification reproducible.
- Keep exactly one current source bundle for each code hash.
- Make the registry rebuildable from Git without relying on process-local state.
- Keep the registry implementation pluggable behind Rust traits.
- Require one testnet payment for each new public verification attempt.
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
- The configured TonCenter v3 provider is trusted to report payment
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

Payment verification always uses TON testnet. The CLI does not accept `--net`
with `--new`.

The ticket always binds a code hash, even when the final `/verify` request also
contains an address. The client computes or resolves the code hash before it
requests the ticket. The backend then uses the address as a consistency check.
If the address changes code before submission, verification fails before the
payment claim.

`POST /api/v1/take-ticket` accepts a code hash. If the code hash is verified,
the endpoint returns the stored bundle metadata. No payment is necessary.

For new code, the endpoint returns:

- The testnet payment address.
- The minimum amount in nanoGRAM.
- The exact comment `acton-verify:v1:<code_hash>`.

The CLI sends a bounceable internal message with this comment. Then it
waits for the finalized recipient transaction and sends its hash to `/verify`.
After `Payment finalized:`, the CLI displays a testnet Actonscan URL. The URL
contains the finalized transaction hash in lowercase hexadecimal form.

The backend gets the transaction from TonCenter v3. It accepts the transaction
only when all these conditions are true:

- The transaction is finalized and is not emulated or aborted.
- The transaction account and incoming destination equal the payment address.
- The incoming message is not bounced.
- The incoming value is not less than the configured minimum.
- The comment equals the ticket comment for the requested code hash.

TonCenter sees the configured payment address, wallet-history reads, and each
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
TonCenter request. Each claim has a generation number. A stale worker cannot
finish a newer claim after its lease expires.

For a successful public verification, the backend stores the payment
transaction hash in lowercase hexadecimal form. The source manifest and lookup
API include this hash. The verifier UI links the hash to Actonscan testnet.

At startup, the payment verifier is not ready. It reads every page of incoming
testnet history up to a captured chain tip. It marks all known ledger entries
as `consumed`, then adds funded historical protocol payments as `consumed`.
The merge never deletes existing replay evidence.

The startup scan ignores payments below the configured minimum. These payments
cannot authorize verification. Payments without the protocol comment also
cannot authorize verification.

During recovery, `/healthz` and `/take-ticket` return `503`. The server retries
a failed scan with an exponential delay of up to 30 seconds.

For unverified code, `/verify` also returns `503` before it claims the payment.
An already-verified lookup can still return successfully during recovery.

Recovery conservatively consumes a payment that reached the payment wallet
before a server crash. This rule also applies when compilation or storage did
not finish before the crash.

Another request can verify the code after ticket issuance but before source
submission. In this race, `/verify` returns `already_verified` without claiming
the payment. The payment cannot verify another code hash and recovery later
marks it as consumed.

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
  <code_hash>/
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
2. Acton sends the code hash to `/take-ticket`.
3. If the code hash is verified, Acton stops successfully without payment.
4. For new code, the backend returns a testnet payment quote.
5. Acton gets wallet approval and sends the payment with the exact comment.
6. Acton waits for the finalized recipient transaction.
7. Acton sends the sources and recipient transaction hash to `/verify`.
8. The backend resolves the target code hash.
9. The backend claims the payment transaction in the ledger.
10. The backend validates source paths and build metadata.
11. The backend compiles the sources and compares both code hashes.
12. If the hashes differ, the response is `mismatch` and no bundle is stored.
13. If the hashes match, the registry stores the payment hash and source bundle.
14. The API returns `match`, `source_bundle_hash`, and `storage_revision`.
15. The backend consumes the payment unless an allowlisted source storage
    failure occurred and the retry budget remains.

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
POST /api/v1/take-ticket
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

Source responses include:

- `code_hash`
- `verified`
- `bundle`, which is `null` when the code hash is not verified

Each source bundle includes `source_bundle_hash`, optional `payment_tx_hash`,
`verified_at`, `storage_revision`, `entrypoint`, a grouped `compiler` object,
optional `source_map`, and source `files`.

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
- Invalid storage configuration, repository integrity errors, Git commit
  errors, and cleanup errors consume the payment.
- A payment can enter `processing` at most three times. Later claims return
  `payment_used` without a TonCenter request.
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
- Payment database state is lost: the ledger is rebuilt from testnet history.
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
