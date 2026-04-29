# Acton Test User-Facing Audit

Date: 2026-04-29

Scope: read-only audit of `acton test`, including command wiring, config merge,
test discovery, reporters, coverage/profiling/trace, mutation testing, debug/UI,
runtime/fork behavior, and the Tolk helper API used from tests.

Method: split the audit across 10 focused workers, then deduplicated and
prioritized findings by user impact. No production code or tests were changed as
part of this audit.

## Priority Guide

- P0: can affect real funds/network, corrupt user files, or make CI report a
  fundamentally wrong result.
- P1: likely user-facing bug that can hide failures, produce wrong artifacts, or
  make common CLI/config behavior misleading.
- P2: diagnostics, consistency, or lower-risk correctness issue.
- P3: improvement or policy/documentation gap.

## P0 Findings

### 2. Mutation testing can mutate out-of-root source files

Status: new candidate, high confidence.

Evidence:
- Absolute contract sources are allowed in config around
  `crates/acton-config/src/config.rs:694`.
- Out-of-root main sources keep an absolute "relative" path in
  `src/commands/test/mutation/mod.rs:789`.
- Mutation workspace code joins that path and writes to it in
  `src/commands/test/mutation/mod.rs:264` and
  `src/commands/test/mutation/mod.rs:300`.

User impact: if a contract uses `src = "/tmp/foo.tolk"` or escapes the project
root, mutation testing can write to the real file instead of an isolated temp
copy. An interrupt or crash could leave user files modified.

Expected: mutation testing should reject out-of-project mutation sources or map
them into a safe sandbox path.

Suggested fix/test:
- Require every mutation source to `strip_prefix(project_root)`.
- Add an integration test with an external source path and assert the command
  rejects it and the external file remains unchanged.

## P1 Findings

### 6. `--manifest-path` can load a manifest while project root stays as cwd

Status: existing confirmed. Also tracked in `ACTON_SMALL_USER_FACING_BUGS.md`.

Evidence:
- Global manifest/project-root handling is in `src/bin/acton.rs:1548` and
  `src/bin/acton.rs:1611`.
- `acton test` uses the configured project root in `src/commands/test/mod.rs:540`.

User impact: `cd /tmp && acton --manifest-path /path/project/Acton.toml test`
can discover tests, resolve outputs, and run builds relative to the caller cwd
instead of the manifest parent.

Expected: when `--manifest-path` is supplied without `--project-root`, project
root defaults to the manifest parent.

Suggested fix/test:
- Derive project root from manifest parent.
- Add a flags integration test that invokes `acton test` from outside the
  project.

### 8. Reporter lifecycle hides or misrepresents failures in CI

Status: fixed. Empty selections and file setup failures now emit synthetic
failed tests before the global reporter finish event, and coverage output write
errors are deferred until after reporter finalization.

Evidence:
- No-match filter finalizes reporters before exiting 1:
  `src/commands/test/mod.rs:674` runs `on_testing_finished`, while
  `src/commands/test/mod.rs:792` exits later. Existing snapshot shows a success
  summary before the error.
- Compile/read errors bail before suite/test reporter events around
  `src/commands/test/mod.rs:1031`; caller prints raw stderr at
  `src/commands/test/mod.rs:653`.
- Coverage file write errors can return before reporter finalization:
  coverage generation starts around `src/commands/test/mod.rs:709`, while
  `reporter_manager.finalize()` is at `src/commands/test/mod.rs:750`.
- JUnit merged output is written in finalization at
  `src/commands/test/reporting/junit.rs:243`.

User impact: TeamCity/JUnit can show a successful or incomplete run while the
process exits 1. Compile diagnostics may be absent from CI test artifacts.

Expected: infrastructure failures are represented as failed suites/tests or
explicit run-aborted events, and reporters are finalized before returning a
non-zero infrastructure error where possible.

Suggested fix/test:
- Added synthetic file-level failed test reports for no-match/empty selections
  and read/compile/setup failures.
- Deferred coverage report write errors until after `ReporterManager::finalize`.
- Added console, TeamCity, and JUnit snapshots for no-match filter and compile
  setup failures, plus a JUnit merge coverage-write regression test.

### 9. Mutation child processes lose command context and can hang on output

Status: new candidates, high confidence.

Evidence:
- Mutation invokes bare `acton build`, `acton test`, and `acton compile` around
  `src/commands/test/mutation/mod.rs:713`,
  `src/commands/test/mutation/mod.rs:335`, and
  `src/commands/test/mutation/mod.rs:1220`.
- Child test forwarding mostly covers filter/include/exclude around
  `src/commands/test/mutation/mod.rs:347`.
- Child stdout/stderr are piped around `src/commands/test/mutation/mod.rs:538`
  and read only after exit around `src/commands/test/mutation/mod.rs:562`.

User impact:
- `acton --project-root /repo test --mutate ...` from another cwd can run child
  commands against the wrong project.
- Fork/fuzz/debug/cache settings can differ between baseline and mutant runs.
- Noisy tests can fill pipe buffers and deadlock workers.

Expected: child commands inherit resolved project context and relevant test
configuration, and child output is drained or bounded.

Suggested fix/test:
- Set child `current_dir(project_root)` and/or pass `--project-root`.
- Forward relevant options such as fork network/block, fuzz seed, clear cache,
  reporter policy, and trace/profile policy where meaningful.
- Drain output concurrently or redirect to null with a bounded failure excerpt.

### 10. Mutation resume and filtering can mislead users

Status: new candidates, medium/high confidence.

Evidence:
- Session metadata stores only contract, source path, and selected IDs around
  `src/commands/test/mutation/session.rs:53`.
- Resume validation checks only those fields around
  `src/commands/test/mutation/session.rs:252`.
- Completed mutation IDs are skipped around `src/commands/test/mutation/mod.rs:910`.
- Resume command omits `--mutation-rules-file` in
  `src/commands/test/mutation/mod.rs:595`.
- Unknown disabled rules are filtered with no validation around
  `src/commands/test/mutation/mod.rs:842`.
- Zero scored mutants produce score `0.0` in
  `src/commands/test/mutation/session.rs:294`, and the threshold check runs
  around `src/commands/test/mutation/mod.rs:1135`.

User impact:
- Resuming after source/custom-rule changes can reuse stale results.
- The printed resume command can resume with the wrong rule set.
- Typos in disabled rule names silently fail to disable rules.
- Empty diff/no-mutant runs can fail as 0 percent instead of giving a clear
  "no selected mutants" policy.

Suggested fix/test:
- Store source hashes, rule fingerprints, and selection/filter fingerprints in
  the session.
- Include `--mutation-rules-file` in the resume command.
- Validate disabled rule names.
- Define and snapshot no-mutants threshold behavior.

### 12. Coverage, trace, and profiling artifacts can be wrong or misleading

Status: new candidates, medium/high confidence.

Evidence:
- `coverage_file` is not normalized in
  `src/commands/test/mod.rs:817`, and generation uses the raw path around
  `src/commands/test/mod.rs:713`.
- Trace filenames use raw test names in `src/commands/test/trace.rs:282` and
  raw contract names in `src/commands/test/trace.rs:276`.
- UI `trace_path` is assigned before skip/todo/config validation in
  `src/commands/test/mod.rs:1118`, but trace dumping happens only after
  emulations exist around `src/commands/test/mod.rs:400`.
- Branch coverage sites are collected from executed VM instructions around
  `src/commands/test/coverage.rs:217`, so never-executed branch sites can be
  absent from the denominator.
- `BuildCache::result_for_code` returns a first hash match around
  `src/context.rs:246`, which coverage/trace/profile then consume.
- Profiling snapshots include regenerated `timestamp` around
  `src/commands/test/profiling.rs:773`, and strict diff compares whole snapshot
  around `src/commands/test/profiling.rs:819`.
- Trace output never cleans or manifests stale generated files.

User impact:
- Coverage files can be written to cwd instead of project root when using
  `--project-root`.
- Tests with `/`, `..`, or duplicate names can overwrite, escape, or collide
  trace files.
- UI can advertise missing traces and return misleading access-denied errors.
- Branch coverage can be inflated.
- Identical contract code hashes can map artifacts to the wrong source/ABI.
- `--fail-on-diff` can fail on timestamp-only snapshot changes.
- Reruns can leave stale traces from previous selections.

Suggested fix/test:
- Normalize `coverage_file` like JUnit and traces.
- Use slug plus stable hash for trace artifact names, preserving original names
  inside JSON.
- Set trace links only after successful dump and return 404 for missing trace
  files.
- Build coverage branch denominator from source/debug metadata, not only hits.
- Track execution identity instead of resolving solely by code hash.
- Ignore snapshot timestamp in drift comparison.
- Clean generated traces or write a run manifest.

### 13. Crypto and formatting helpers can return wrong values

Status: mixed existing confirmed and new candidates.

Evidence:
- `rawSign` and data conversion use `BigInt::to_bytes_be` without validating
  sign around `src/ffi/crypto.rs:76` and `src/ffi/crypto.rs:95`.
- Existing bug tests in
  `tests/integration/test_runner/crypto_sign_is_hash_sensitive_for_different_cells_in_fixture_project_tests.rs`
  show positive and negative values collapsing.
- `{:ton}` formatting uses `BigInt::to_f64` around `src/ffi/io.rs:233`.

User impact:
- Distinct logical inputs can produce identical signatures.
- Large TON values can be rounded in user-visible test output.

Expected:
- Reject negative/out-of-range values or define exact unsigned conversion.
- Format TON amounts by integer division/remainder, not floating point.

Suggested fix/test:
- Validate `0 <= value < 2^256` for crypto inputs if that is the intended type.
- Add snapshots for negative, positive, oversized, and boundary values.
- Add exact `{:ton}` snapshots for `i64::MAX` and larger bigint values.

## P2 Findings

### 14. Test discovery and input validation diagnostics are late or unclear

Findings:
- Missing/invalid test path is checked after `build_cmd`, so broken projects can
  show compile errors before "file not found" (`src/commands/test/mod.rs:545`,
  `src/commands/test/mod.rs:555`).
- Debug/UI port conflicts are also checked after build
  (`src/commands/test/mod.rs:545`, `src/commands/test/mod.rs:592`).
- Invalid `--filter` regex is compiled per file, so it can be hidden when
  include/exclude selects no files (`src/commands/test/mod.rs:1068`).
- Include/exclude globs are project-root-relative, but docs do not make the base
  obvious.
- Explicit file paths bypass include/exclude globs.
- Invalid glob errors do not name whether the bad value came from `--include` or
  `--exclude`.

Suggested improvements:
- Preflight path, regex, globs, and listener ports before build.
- Document glob base, or support both project-root and selected-dir relative
  matching with clear diagnostics.
- Decide whether globs should affect explicit file targets, then snapshot the
  policy.

### 16. Fuzz annotations and fuzz failure reporting need sharper diagnostics

Findings:
- Malformed `@test.fuzz(...)` can lose the fact that the annotation was present,
  producing generic "requires @test.fuzz" errors (`src/commands/test/annotations.rs:155`).
- Duplicate fuzz annotations replace instead of merging or rejecting.
- Reported "Fuzz case" is accepted-run number, while RNG attempts include
  rejected `assume(...)` inputs (`src/commands/test/fuzz.rs:107`,
  `src/commands/test/fuzz.rs:145`).
- `[test.fuzz]` unknown or underscored keys can be silently ignored.
- Assume-budget exhaustion drops the last rejected input and custom assume
  message.

Suggested improvements:
- Make annotation parsing return validation diagnostics with locations.
- Reject or explicitly merge duplicate fuzz annotations.
- Report both accepted case number and attempt/rejection count.
- Add `deny_unknown_fields` or explicit metadata validation for fuzz config.



### 18. Smaller assertion/diagnostic issues

Findings:
- Gas-limit failures set only the message and lose source location around
  `src/commands/test/mod.rs:1269`.
- `--backtrace full` still drops useful caller details for get-method transport
  errors; console prints only `error.error` around
  `src/commands/test/reporting/console.rs:285`.
- Address/code display names live on `TestRunner` and can leak between tests via
  `known_addresses`/`known_code_cells`.

Suggested improvements:
- Swap map length assertion argument order.
- Attach test descriptor location to gas-limit failures.
- Carry caller trace for `GetMethodResult::Error`.
- Reset or snapshot/restore display-name registries per test.
