# acton-test(1)

## NAME

acton-test --- Discover and run Tolk smart contract tests

## SYNOPSIS

`acton test` [_options_] [_path_]

## DESCRIPTION

Discover and execute tests from `.test.tolk` files.

The test runner supports filtering, multiple reporters, coverage collection,
gas snapshots, mutation testing, trace export, remote-state fork mode, and a
browser UI for exploring results.

## OPTIONS

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
{{/option}}

{{/options}}

### Debugging Options

{{#options}}

{{#option "`--debug`" }}
Enable debug mode.
{{/option}}

{{#option "`--debug-port` _port_" }}
Debug server port.
{{/option}}

{{#option "`--backtrace` _level_" }}
Enable execution backtraces.

Currently supported value: `full`.
{{/option}}

{{/options}}

### Coverage Options

{{#options}}

{{#option "`--coverage`" }}
Generate a coverage profile.
{{/option}}

{{#option "`--coverage-format` _format_" }}
Coverage output format.

Possible values: `lcov`, `text`
{{/option}}

{{#option "`--coverage-file` _path_" }}
Output file for the coverage report.
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
{{/option}}

{{#option "`--junit-merge`" }}
Merge all test suites into a single JUnit XML file.
{{/option}}

{{#option "`--ui`" }}
Open test results in a browser UI.
{{/option}}

{{#option "`--ui-port` _port_" }}
Port for the browser UI server.
{{/option}}

{{/options}}

### Cache Options

{{#options}}

{{#option "`--clear-cache`" }}
Clear the compilation cache before running tests.
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

{{#option "`--api-key` _key_" }}
TonCenter API key for blockchain queries.
{{/option}}

{{/options}}

### Tracing Options

{{#options}}

{{#option "`--save-test-trace` [_dir_]" }}
Save transaction traces to a directory.

If passed without a value, Acton uses `.acton/traces`.
{{/option}}

{{/options}}

### Mutation Testing Options

{{#options}}

{{#option "`--mutate`" }}
Run tests in mutation testing mode.
{{/option}}

{{#option "`--mutate-contract` _contract-id_" }}
Contract ID to mutate during mutation testing.
{{/option}}

{{#option "`--disable-rule` _rule_" }}
Disable a specific mutation rule.

May be passed multiple times.
{{/option}}

{{/options}}

### Display Options

{{> options-display }}

### Project Options

{{> options-project-resolved }}

## DISCOVERY

Acton discovers tests by finding files that end with `.test.tolk`.

- If `_path_` is omitted, discovery starts at the resolved project root
- If `_path_` is a directory, search is recursive
- Relative `_path_` values are resolved from the current working directory

## CONFIGURATION

Defaults can be configured in `Acton.toml`:

```toml
[test]
reporter = ["console"]
coverage = true
filter = "test-.*"
junit-path = "reports"
junit-merge = false
```

CLI flags override config values for the current invocation.

## NOTES

- If both `--snapshot` and `--baseline-snapshot` are provided, Acton runs in
  comparison mode and does not overwrite the snapshot file
- `--fail-on-diff` requires `--baseline-snapshot`
- The UI and trace export features are useful for debugging failing tests and
  inspecting transaction trees

## EXAMPLES

1. Run all tests:

   ```bash
   acton test
   ```

2. Filter tests by name:

   ```bash
   acton test --filter "wallet.*"
   ```

3. Generate coverage and JUnit output:

   ```bash
   acton test --coverage --coverage-format lcov --reporter junit --junit-path test-results
   ```

4. Compare gas usage against a baseline:

   ```bash
   acton test --baseline-snapshot .acton/gas-baseline.json --fail-on-diff
   ```

5. Run mutation testing for one contract:

   ```bash
   acton test --mutate --mutate-contract wallet
   ```

## SEE ALSO

- `acton help wrapper`
- [Test runner guide](https://ton-blockchain.github.io/acton/docs/test-runner)
