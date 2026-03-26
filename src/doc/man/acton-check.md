# acton-check(1)

## NAME

acton-check --- Lint Tolk contracts and files

## SYNOPSIS

`acton check` [_options_] [_target_]

## DESCRIPTION

Run the Tolk linter on the whole project, a specific contract ID from
`Acton.toml`, or a single `.tolk` file.

By default, Acton checks contracts from `Acton.toml` together with workspace
test files. The command also supports machine-readable output formats and rule
explanations.

## OPTIONS

### Check Options

{{#options}}

{{#option "_target_" }}
Contract ID from `Acton.toml` or path to a `.tolk` file.
{{/option}}

{{#option "`--fix`" }}
Apply available fixes automatically.

Works only in plain output mode.
{{/option}}

{{#option "`--output-format` _format_" }}
Output format for diagnostics.

Possible values: `plain`, `json`, `sarif`, `github`, `gitlab`
{{/option}}

{{#option "`--output-file` _path_" }}
Write output to a file instead of standard output.
{{/option}}

{{#option "`--enable-only` _code_[,_code_...]_" }}
Enable only selected lint rules, for example `E001,S001`.
{{/option}}

{{#option "`--explain` _code_" }}
Print an explanation for a lint rule.
{{/option}}

{{/options}}

### Display Options

{{> options-display }}

### Project Options

{{> options-project-resolved }}

## BEHAVIOR

- `--fix` applies edits only in `plain` output mode
- non-plain output formats report diagnostics without rewriting source files
- `github` emits GitHub Actions workflow annotations
- output format resolution is: CLI flag, then `[lint].output-format`, then
  default `plain`
- `--output-file` is not valid with `plain` format
- single-file mode relaxes `E014 (acton-import-in-contract)`
- targets ending with `.tolk` are treated as file paths; other targets are
  resolved as contract IDs from `Acton.toml`
- use an explicit `.tolk` path such as `./contracts/counter.tolk` when a name
  could be mistaken for a contract ID
- inline suppressions use `// acton-disable-next-line E001,S001`
- `--fix` applies only linter-provided fixes; diagnostics without a safe fix
  remain in the report

## EXIT STATUS

- `0`: No lint errors were reported, warning thresholds were respected, and any
  requested autofixes completed successfully.
- `1`: Lint errors were found, warning limits were exceeded, autofix failed, or
  configuration/target resolution failed.

## EXAMPLES

1. Check the whole project:

   ```bash
   acton check
   ```

2. Check a specific contract by ID:

   ```bash
   acton check counter
   ```

3. Check one file:

   ```bash
   acton check counter.tolk
   ```

4. Emit SARIF to a file:

   ```bash
   acton check --output-format sarif --output-file .acton/reports/lint.sarif
   ```

5. Run only selected rules:

   ```bash
   acton check --enable-only E001,S001
   ```

6. Emit GitHub Actions annotations in CI:

   ```bash
   acton check --output-format github
   ```

7. Apply local autofixes where available:

   ```bash
   acton check --fix
   ```

## SEE ALSO

- `acton help fmt`
- [Linting guide](https://ton-blockchain.github.io/acton/docs/linting)
- [Linter rules](https://ton-blockchain.github.io/acton/docs/linting/rules)
