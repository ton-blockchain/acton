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

## Goals

- Allow developers to publish source code that reproducibly compiles to a known
  TON code hash.
- Keep the public lookup key simple: `code_hash`.
- Store enough compilation metadata to make verification reproducible.
- Keep exactly one current source bundle for each code hash.
- Make the registry rebuildable from Git without relying on process-local state.
- Keep the registry implementation pluggable behind Rust traits.

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
- Users who need stronger assurance can download a bundle, recompute its
  `source_bundle_hash`, recompile it, and compare the resulting `code_hash`.

This is not a trustless proof system. The product should use wording such as
"verified by this source registry" and avoid claiming external anchoring.

## Architecture

The system has three main parts:

1. Verification backend.
2. Source storage.
3. Verification registry.

### Verification Backend

The backend receives verification requests from developers.

Input:

- Target `code_hash`, or an address whose current code hash can be read from
  TON.
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

## Source Bundle

A source bundle is the reproducible unit of verification.

It contains:

- All source files required by the build.
- A manifest.
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

1. Developer submits target and sources.
2. Backend resolves the target `code_hash`.
3. If the code hash is already verified, the API immediately returns
   `verification_result=already_verified` with the stored `source_bundle_hash`.
4. Backend validates source paths and build metadata.
5. Backend compiles sources with the selected compiler.
6. Backend compares compiled hash with target hash.
7. If hashes differ, response is `mismatch` and nothing is stored.
8. If hashes match, backend computes `source_bundle_hash`.
9. Registry stores the bundle in Git.
10. Registry validates that the stored bundle is indexable.
11. Registry upserts the bundle into SQLite.
12. API returns `verification_result=match`, `source_bundle_hash`, and
    `storage_revision`.

## Lookup Flow

To check whether an address uses verified code:

1. Read the current code hash of the address from TON.
2. Query the registry by `code_hash`.
3. If a valid source bundle exists, the code hash is verified.
4. Return the source bundle and build metadata.
5. A user can independently recompute file hashes and recompile the bundle.

## API Model

Current public endpoints:

```text
POST /api/v1/verify
GET /api/v1/openapi.json
GET /api/v1/verification/status?code_hash=...
GET /api/v1/verification/status?address=...
GET /api/v1/verification/source?code_hash=...
GET /api/v1/verification/source?address=...
GET /api/v1/last_verified?limit=50&offset=0
GET /api/v1/abi?code_hash=...
```

Status responses include:

- `code_hash`
- `verified`

Source responses include:

- `code_hash`
- `verified`
- `bundle`, which is `null` when the code hash is not verified

Each source bundle includes `source_bundle_hash`, `verified_at`,
`storage_revision`, `entrypoint`, a grouped `compiler` object, and source
`files`.

Last verified and ABI requests accept `limit` and `offset`, but responses only
include `items`. Last verified items are ordered by recent verification time;
ABI items contain the `code_hash` and parsed `abi` JSON.

## Failure Handling

Important cases:

- Compilation fails: no storage write.
- Hash mismatch: no storage write.
- Git write fails: verification request fails.
- Stored bundle cannot be re-read or validated: verification request fails.
- Backend process state is lost: registry can be rebuilt from Git.
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
