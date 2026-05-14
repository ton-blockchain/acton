# acton-test(1)

## Name

acton-test --- Discover and run Tolk smart contract tests

## Synopsis

`acton test` [_options_] [_path_]

## Description

Discover and execute tests from `.test.tolk` files.

The test runner supports filtering, multiple reporters, coverage collection,
gas snapshots, mutation testing, trace export, remote-state fork mode, and a
browser UI for exploring results.

## Options

### Discovery Options

{{#options}}

{{#option "_path_" }}
Test file or directory to run.

If omitted, Acton discovers tests from the resolved project root.
{{/option}}

{{#option "`-f`, `--filter` _pattern_" }}
Filter tests by regular expression.
{{/option}}

{{#option "`--exclude` _glob_" }}
Exclude test files or directories matching a glob pattern.

May be passed multiple times.
{{/option}}

{{#option "`--include` _glob_" }}
Include only test files or directories matching a glob pattern.

May be passed multiple times.
{{/option}}

{{/options}}

### Execution Options

{{#options}}

{{#option "`--fail-fast`" }}
Stop after the first failing test.

Defaults to `[test].fail-fast`, or `false` when it is not configured.
{{/option}}

{{#option "`--fuzz-seed` _seed_" }}
Seed for reproducible fuzz runs.
{{/option}}

{{#option "`--verbose`" }}
Enable executor debug logs at verbosity level `1`.

Currently only level `1` is supported. Pass `--verbose` at most once.
Use this for low-level executor output such as `debug.dumpStack()`. For richer
debug output, use `--backtrace full` or `--debug`.
{{/option}}

{{/options}}

### Debugging Options

{{#options}}

{{#option "`--debug`" }}
Enable debug mode.

This also raises executor verbosity to collect full source locations and stack
context for live stepping.
{{/option}}

{{#option "`--debug-port` _port_" }}
Debug server port.
{{/option}}

{{#option "`--backtrace` _level_" }}
Enable execution backtraces.

Currently supported value: `full`.
Full backtraces also raise executor verbosity to collect source locations and
call stacks without opening the debugger.
{{/option}}

{{/options}}

### Coverage Options

{{#options}}

{{#option "`--coverage`" }}
Generate a coverage profile.

Coverage also raises internal executor verbosity enough to map executed
locations back to source files and lines, but it does not enable stepping or
print backtraces by itself.
{{/option}}

{{#option "`--coverage-format` _format_" }}
Coverage output format.

Possible values: `lcov`, `text`
{{/option}}

{{#option "`--coverage-file` _path_" }}
Output file for the coverage report.
{{/option}}

{{#option "`--coverage-minimum-percent` _percent_" }}
Fail when the final coverage score is below this percentage.

Valid range: `0` to `100`.

Ignored with `--ui`.
{{/option}}

{{#option "`--coverage-include-wrappers`" }}
Include files from the `@wrappers` mapping in coverage reports.
{{/option}}

{{#option "`--coverage-include-tests`" }}
Include `.test.tolk` files in coverage reports.
{{/option}}

{{/options}}

### Profiling Options

{{#options}}

{{#option "`--snapshot` _path_" }}
Write gas usage statistics to a JSON snapshot file.
{{/option}}

{{#option "`--baseline-snapshot` _path_" }}
Compare gas usage with a baseline snapshot.
{{/option}}

{{#option "`--fail-on-diff`" }}
Exit non-zero when the current run differs from the baseline snapshot.

Requires `--baseline-snapshot`.
{{/option}}

{{/options}}

### Reporting Options

{{#options}}

{{#option "`--reporter` _format_[,_format_...]_" }}
Report format list.

Possible values: `console`, `teamcity`, `junit`, `dot`
{{/option}}

{{#option "`--show-bodies`" }}
Show decoded message bodies in printed transaction trees when ABI is known.
{{/option}}

{{#option "`--junit-path` _path_" }}
Output directory for JUnit XML reports.

Defaults to `[test].junit-path`, or `test-results` when it is not configured.
{{/option}}

{{#option "`--junit-merge`" }}
Merge all test suites into a single JUnit XML file.
{{/option}}

{{#option "`--ui`" }}
Open test results in a browser UI.
{{/option}}

{{#option "`--ui-port` _port_" }}
Port for the browser UI server.

Defaults to `[test].ui-port`, or `12344` when it is not configured.
{{/option}}

{{/options}}

### Cache Options

{{#options}}

{{#option "`--clear-cache`" }}
Clear the compilation cache before running tests.

Defaults to `false`.
{{/option}}

{{/options}}

### Remote Options

{{#options}}

{{#option "`--fork-net` _network_" }}
Fork remote blockchain state for account resolution.
{{/option}}

{{#option "`--fork-block-number` _seqno_" }}
Historical block sequence number to fork from.
{{/option}}

{{/options}}

### Tracing Options

{{#options}}

{{#option "`--save-test-trace` [_dir_]" }}
Save transaction traces to a directory.

If passed without a value, Acton uses `build/traces`.
Acton writes one `<test-name>_trace.json` file per executed test and stores
shared contract metadata under `contracts/<contract-name>.json`.
These filenames are derived only from the test name and contract name, so
duplicate names overwrite earlier artifacts in the same output directory.
Relative paths are resolved from the project root.
{{/option}}

{{/options}}

## TonCenter API Keys

When tests fork from the built-in `mainnet`/`testnet` backends, authenticated
requests read `TONCENTER_MAINNET_API_KEY` or `TONCENTER_TESTNET_API_KEY`.

When tests fork from `custom:<name>`, Acton reads `<NORMALIZED_NAME>_API_KEY`.
Custom network names are uppercased and non-alphanumeric characters are
replaced with `_`, so `custom:mock-remote` becomes `MOCK_REMOTE_API_KEY`.

Acton loads `.env` automatically, so the simplest setup during project work is
usually to keep these keys there and use shell environment variables only for
one-off overrides or CI.

### Mutation Testing Options

{{#options}}

{{#option "`--mutate`" }}
Run tests in mutation testing mode.
{{/option}}

{{#option "`--mutate-contract` _contract-name_" }}
Contract name to mutate during mutation testing.
{{/option}}

{{#option "`--mutation-diff` _mode_" }}
Limit mutation testing to changed lines in the selected diff scope.

Accepted values: `worktree`, `ref`, `branch`.

- `worktree` compares the current worktree with `HEAD` and includes untracked
  files
- `ref` compares against the explicit ref passed with `--mutation-diff-ref`
- `branch` compares against the merge-base with the current branch upstream, or
  against `--mutation-diff-ref` when provided
{{/option}}

{{#option "`--mutation-diff-ref` _ref_" }}
Base ref used by diff-based mutation testing modes.

Required with `--mutation-diff ref`. Optional with `--mutation-diff branch`.
Use it with `branch` when the current branch has no upstream or when you want a
different comparison base such as `origin/main`.
{{/option}}

{{#option "`--mutation-levels` _level[,level...]_" }}
Run only selected mutation levels.

Accepted values: `critical`, `major`, `minor`.
Useful for faster local runs such as `critical,major`.
{{/option}}

{{#option "`--mutation-rules-file` _path_" }}
Load custom query-based mutation rules from a JSON file.

Custom rules are merged with built-in rules. If a custom rule uses the same
rule ID, it overrides the built-in one. Relative paths are resolved from the
project root.
{{/option}}

{{#option "`--mutation-session-id` _id_" }}
Use a specific mutation session ID for progress logging and resume.

Acton writes append-only JSON Lines progress to
`build/mutation-sessions/<ID>.jsonl`. Re-run with the same session ID and the
same mutation filters to continue an unfinished session.
If you stop the run with `Ctrl+C`, Acton prints a resume command that includes
the same session ID.
{{/option}}

{{#option "`--mutation-workers` _count_" }}
Override the number of mutation workers.

By default, Acton uses the host's available parallelism. Each worker keeps its
own isolated mutation workspace and reuses it across multiple mutants.
{{/option}}

{{#option "`--mutation-id` _id_" }}
Run only specific mutation IDs from a previous mutation report.

Pass the mutation number shown in the report, and use the same mutation filters
such as `--mutation-diff`, `--mutation-levels`, and `--mutation-disable-rules` as the
original run. May be passed multiple times or as a comma-separated list.
{{/option}}

{{#option "`--mutation-minimum-percent` _percent_" }}
Fail if mutation score is below this percentage.

Valid range: `0..=100`.
{{/option}}

{{#option "`--mutation-disable-rules` _rule_" }}
Disable a specific mutation rule.

May be passed multiple times.
{{/option}}

{{/options}}

### Display Options

{{> options-display }}

### Project Options

{{> options-project-resolved }}

## Discovery

Acton discovers tests by finding files that end with `.test.tolk`.

- If `_path_` is omitted, discovery starts at the resolved project root
- If `_path_` is a directory, search is recursive
- Relative `_path_` values are resolved from the current working directory

## Reporting And Artifacts

- `--reporter` on the CLI overrides `[test].reporter` for the current run
- `--ui` adds the browser UI in addition to text reporters
- `--coverage --ui` adds a `Coverage` tab to the browser UI for browsing
  coverage summaries, files, and annotated source
- `--junit-path` matters when the JUnit reporter is enabled; it defaults to
  `[test].junit-path`, or `test-results` when it is not configured
- executor debug logs are hidden by default; re-run with `--verbose` when you need
  level-1 executor output such as `debug.dumpStack()`
- `--verbose` is only low-level executor logging; `--coverage` also collects
  source-location data, while `--backtrace full` and `--debug` collect richer
  location and stack data
- `--coverage-file` matters only with `--coverage`; without an explicit path,
  Acton writes `lcov.info` for `lcov` and `coverage.txt` for `text`
- coverage summaries show line and branch columns plus a blended `Score`
- LCOV export includes `BRDA`, `BRF`, and `BRH` records when branch sites were
  observed for a file
- `--coverage-minimum-percent` checks the final blended `Score` shown in the
  summary, not just `% Lines`
- coverage excludes files under `tests/`, `.test.tolk` files, and `@wrappers`
  sources by default;
  use `--coverage-include-tests` or `--coverage-include-wrappers` to opt in
- `--save-test-trace` without a value writes traces to `build/traces`
- each executed test produces its own `<test-name>_trace.json` artifact
- exported bundles also include `contracts/<contract-name>.json` files with
  code, source maps, and ABI metadata reused across tests in the same bundle
- trace chains default to names like `Trace 1`; use `txs.giveName("...")` when
  you want stable human-readable names in exported artifacts and the Test UI
- when you split execution with `testing.createTraceIterationCursor()`,
  exported traces still merge batches that belong to the same logical root
  transaction chain
- `--ui` also enables the default trace bundle directory when
  `--save-test-trace` is otherwise absent
- gas snapshot files are written only to the explicit paths passed to
  `--snapshot` or `--baseline-snapshot`
- `[test.fuzz]` applies only to parameterized tests that explicitly opt in with
  `@test.fuzz`, `@test.fuzz(<runs>)`, or `@test.fuzz({ ... })`
- `--fuzz-seed` overrides `[test.fuzz].seed` for the current run
- fuzz tests show the seed in console output; if `[test.fuzz].seed` is omitted,
  Acton picks a new seed for each `acton test` run

## Dot Reporter

Use `dot` when you want compact progress output but still need full failure
diagnostics at the end of the run:

```bash
acton test --reporter dot
```

The progress line uses one character per test:

- `·` passed
- `x` failed
- `○` skipped
- `□` todo

When a test fails, the dot reporter prints the same important diagnostics as the
console reporter after the progress line: assertion diffs, transaction trees,
get-method errors, fuzz seed and input values, stdout/stderr, and source
locations. With `--backtrace full`, runtime failures also include backtrace
frames:

```bash
acton test --reporter dot --backtrace full
```

If coverage is enabled, dot failure details are printed before the coverage
summary so the failing test remains visible near the progress output.

## Configuration

Defaults can be configured in `Acton.toml`:

```acton-toml title="Acton.toml"
[test]
reporter = ["console"]
filter = ".*jetton.*"
junit-path = "reports"
junit-merge = false

[test.fuzz]
runs = 512
max-test-rejects = 4096
seed = 42

[test.coverage]
enabled = true
# valid range: 0..=100
minimum-percent = 85
include-tests = true
include-wrappers = true

[test.mutation]
diff = "branch"
diff-ref = "origin/main"
mutation-levels = ["critical", "major"]
minimum-percent = 85
disable-rules = ["replace_plus_with_minus"]
```

CLI flags override config values for the current invocation.

## Notes

- If both `--snapshot` and `--baseline-snapshot` are provided, Acton runs in
  comparison mode and does not overwrite the snapshot file
- `--fail-on-diff` requires `--baseline-snapshot`
- The UI and trace export features are useful for debugging failing tests and
  inspecting transaction trees
- You can combine `--coverage` and `--ui` to inspect the current run's coverage
  directly in the browser UI
- `--coverage-minimum-percent` and `[test.coverage].minimum-percent` are ignored
  when `--ui` is enabled
- `--fork-net` keeps execution local while resolving blockchain state remotely
- `--mutation-diff worktree` is intended for uncommitted local changes
- `--mutation-diff ref` requires `--mutation-diff-ref`
- `--mutation-diff branch` uses the upstream branch merge-base by default
- `--mutation-rules-file` loads custom query-based rules from JSON and custom
  rules override built-in rules with the same ID
- `--mutation-session-id` writes append-only JSONL progress to
  `build/mutation-sessions/<ID>.jsonl`
- `--mutation-workers` defaults to the host's available parallelism; each
  worker reuses its own isolated mutation workspace
- pressing `Ctrl+C` during mutation testing stops the run without finalizing the
  session and prints a resume command for the same session ID
- `--mutation-id` expects mutation numbers from a previous run with the same mutation
  filters and session selection
- mutation scores in filtered runs only cover the selected mutants
- `--mutation-minimum-percent` and `[test.mutation].minimum-percent` apply to
  that filtered mutation score after compile errors are excluded

## Exit Status

- `0`: All selected tests passed, or a non-mutating reporting mode completed
  successfully.
- `1`: At least one test failed, profiling drift was detected with
  `--fail-on-diff`, line coverage was below the configured minimum in non-UI
  coverage mode, mutation score was below the configured minimum in mutation
  mode, no tests matched after filtering, or infrastructure such as
  compilation, trace export, UI startup, or remote-state resolution failed.

## Examples

1. Run all tests:

   ```bash
   acton test
   ```

2. Filter tests by name:

   ```bash
   acton test --filter "wallet.*"
   ```

3. Show executor debug logs from `debug.*` helpers:

   ```bash
   acton test --verbose --filter "debug.*"
   ```

4. Generate coverage and JUnit output:

   ```bash
   acton test --coverage --coverage-format lcov --reporter junit \
                                                --junit-path test-results
   ```

5. Fail the run when line coverage drops below 85%:

   ```bash
   acton test --coverage --coverage-minimum-percent 85
   ```

6. Compare gas usage against a baseline:

   ```bash
   acton test --baseline-snapshot build/gas-baseline.json --fail-on-diff
   ```

7. Run mutation testing for one contract:

   ```bash
   acton test --mutate --mutate-contract Wallet
   ```

8. Run mutation testing only for changed lines in the current worktree:

   ```bash
   acton test --mutate --mutate-contract Wallet --mutation-diff worktree
   ```

9. Run mutation testing for selected levels on the current branch:

   ```bash
   acton test --mutate --mutate-contract Wallet --mutation-diff branch \
                                                --mutation-levels critical,major
   ```

10. Re-run one specific mutant from a previous report:

   ```bash
   acton test --mutate --mutate-contract Wallet --mutation-id 2
   ```

11. Resume an unfinished mutation session:

   ```bash
   acton test --mutate --mutate-contract Wallet --mutation-session-id wallet-pr-42 \
                                                --mutation-diff worktree
   ```

12. Fail the run when mutation score drops below 85%:

   ```bash
   acton test --mutate --mutate-contract Wallet --mutation-minimum-percent 85
   ```

13. Limit mutation testing to four workers:

   ```bash
   acton test --mutate --mutate-contract Wallet --mutation-workers 4
   ```

14. Debug a forked-state failure with traces and the UI:

   ```bash
   acton test tests/wallet.test.tolk --fork-net testnet \
                                     --fork-block-number 55000000 \
                                     --save-test-trace --ui
   ```

15. Enforce a gas baseline in CI:

   ```bash
   acton test --baseline-snapshot build/gas-baseline.json --fail-on-diff \
                                                          --reporter console,junit
   ```

## See Also

- `acton help wrapper`
- [Test runner guide](https://ton-blockchain.github.io/acton/docs/testing/overview)
