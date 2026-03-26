# acton-fmt(1)

## NAME

acton-fmt --- Format Tolk source files

## SYNOPSIS

`acton fmt` [_options_] [_paths_...]

## DESCRIPTION

Format `.tolk` files using the built-in Tolk formatter.

The command can rewrite files in place or run in `--check` mode for CI and
pre-commit validation. If no `_paths_` are provided, Acton scans the resolved
project root recursively.

## OPTIONS

### Format Options

{{#options}}

{{#option "_paths_..." }}
Files or directories to format.

If omitted, Acton scans the project root.
{{/option}}

{{#option "`--check`" }}
Check formatting without rewriting files.

In this mode Acton prints diffs for mismatches and exits non-zero.
{{/option}}

{{/options}}

### Display Options

{{> options-display }}

### Project Options

{{> options-project-resolved }}

## BEHAVIOR

- Only `.tolk` files are formatted
- Directory traversal is recursive
- Built-in ignore globs always apply, including `node_modules`, `.git`,
  `target`, and `.acton`
- explicit file arguments are formatted even if they would match `[fmt].ignore`
- directory traversal applies `[fmt].ignore` and built-in excludes to relative
  and absolute paths
- Syntax errors are reported as diagnostics and cause a non-zero exit
- `--check` prints a unified diff with three lines of context for each changed
  file

## CONFIGURATION

`acton fmt` reads defaults from `[fmt]` in `Acton.toml`:

```toml
[fmt]
width = 100
ignore = ["contracts/generated/*.tolk"]
separate-import-groups = true
```

Useful fields include:

- `width` for maximum formatted line width
- `ignore` for additional exclude globs
- `separate-import-groups` for blank lines between import groups

## IMPORT SORTING

Imports are sorted by group in this order:

1. `@stdlib`
2. `@acton`
3. other `@...` imports
4. plain imports
5. `./...`
6. `../...`

Within each group, imports are sorted lexicographically.

## EXIT STATUS

- `0`: All requested files were formatted successfully, or `--check` found no
  formatting differences.
- `1`: Files needed formatting in `--check` mode, syntax errors prevented
  formatting, or path resolution failed.

## EXAMPLES

1. Format all Tolk files in the project:

   ```bash
   acton fmt
   ```

2. Format selected paths:

   ```bash
   acton fmt contracts scripts/deploy.tolk
   ```

3. Validate formatting without writing files:

   ```bash
   acton fmt --check
   ```

4. Run against another project root:

   ```bash
   acton --project-root ../my-project fmt --check
   ```

5. Check import grouping after enabling blank lines between groups:

   ```bash
   acton fmt contracts/main.tolk --check
   ```

## SEE ALSO

- `acton help check`
- [Formatting guide](https://ton-blockchain.github.io/acton/docs/commands/fmt)
