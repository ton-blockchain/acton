# acton-check(1)

## Name

acton-check --- Lint Tolk contracts and files

## Synopsis

`acton check` [_options_] [_target_]

## Description

Run the Tolk linter on the whole project, a specific contract name from
`Acton.toml`, or a single `.tolk` file.

By default, Acton checks contracts from `Acton.toml` together with workspace
test files and standalone script roots that define `main()`. The command also
supports machine-readable output formats and rule explanations.

## Options

### Check Options

{{#options}}

{{#option "_target_" }}
Contract name from `Acton.toml` or path to a `.tolk` file.
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

## Behavior

- `--list-lint-rules` is a hidden machine-readable helper that prints JSON
  entries with rule `name` and markdown `description`
- for a human-readable catalog with rule codes, lifecycle status, and quick-fix
  availability, use the web rules index
- `--fix` applies edits only in `plain` output mode
- `--fix` rewrites files only for fixes marked as automatically applicable
- when a diagnostic offers multiple fixes, `--fix` applies only the first one
- non-plain output formats report diagnostics without rewriting source files
- `github` emits GitHub Actions workflow annotations
- output format resolution is: CLI flag, then `[lint].output-format`, then
  default `plain`
- `--output-file` is not valid with `plain` format
- project-wide mode includes workspace `*.test.tolk` files and standalone
  script roots that define `main()`
- project-wide discovery skips built-in directories such as `.git`, `.github`,
  `.idea`, `.acton`, `node_modules`, `target`, `tolk-stdlib`, `.codex`, and
  `.claude`
- contracts with non-`.tolk` sources such as precompiled `.boc` entries are not
  lint roots
- single-file mode relaxes `E014 (acton-import-in-contract)`
- targets ending with `.tolk` are treated as file paths; other targets are
  resolved as contract names from `Acton.toml`
- use an explicit `.tolk` path such as `./contracts/Counter.tolk` when a name
  could be mistaken for a contract name
- inline suppressions use rule names, for example
  `// check-disable-next-line unused-variable, write-only-variable`
- suppressions apply only to the immediately following line and match
  diagnostic names, including `compiler-error` and `parse-error`; diagnostic
  codes such as `C001` are not matched
- `--fix` applies only linter-provided fixes; diagnostics without a safe fix
  remain in the report
- `--enable-only` is applied after config is loaded: selected rules keep their
  configured severity, selected rules that were `allow` are re-enabled at the
  default severity, and every unselected rule is forced to `allow`
- `--enable-only` selectors must resolve to exactly one rule; in practice, use
  full rule codes such as `E003` for stability
- excluded files still participate in import/type resolution, but lint
  diagnostics for excluded non-root files are filtered out
- explicit targets still run even if they match `[lint].exclude`
- in project-wide runs, excluded contract roots are skipped entirely, so their
  compiler errors and lint diagnostics are not reported
- compiler errors are still shown for roots that are actually checked,
  including explicit targets
- `--fix` can still exit with status `1` when unfixed diagnostics or warning
  threshold violations remain after rewriting

## Exit Status

- `0`: No lint errors were reported, warning thresholds were respected, and any
  requested autofixes completed successfully.
- `1`: Lint errors were found, warning limits were exceeded, autofix failed, or
  configuration/target resolution failed.

## Examples

1. Check the whole project:

   ```bash
   acton check
   ```

2. Check a specific contract by name:

   ```bash
   acton check Counter
   ```

3. Check one file:

   ```bash
   acton check ./contracts/Counter.tolk
   ```

4. Emit SARIF to a file:

   ```bash
   acton check --output-format sarif --output-file build/reports/lint.sarif
   ```

5. Run only selected rules:

   ```bash
   acton check --enable-only E001,S001
   ```

6. List rule metadata as JSON:

   ```bash
   acton check --list-lint-rules
   ```

7. Emit GitHub Actions annotations in CI:

   ```bash
   acton check --output-format github
   ```

8. Apply local autofixes where available:

   ```bash
   acton check --fix
   ```

## See Also

- `acton help fmt`
- [Linting guide](https://ton-blockchain.github.io/acton/docs/linting/overview)
- [Linter rules](https://ton-blockchain.github.io/acton/docs/rules/overview)
