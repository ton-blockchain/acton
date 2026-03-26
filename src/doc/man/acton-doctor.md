# acton-doctor(1)

## NAME

acton-doctor --- Inspect the resolved Acton project environment

## SYNOPSIS

`acton doctor` [_options_]

## DESCRIPTION

Print a diagnostic report for the current Acton environment.

The report is intended to help debug project resolution, manifest parsing,
wallet and library overlays, bundled standard-library state, logging paths, and
relevant environment variables.

## DISPLAY OPTIONS

{{> options-display }}

## PROJECT OPTIONS

{{> options-project-resolved }}

## REPORT SECTIONS

`acton doctor` prints:

- version and build metadata
- resolved paths for the project, manifest, `.acton`, cache, wallets, and
  libraries
- `Acton.toml` existence and parse status
- wallet and library overlay load status
- bundled stdlib health and version
- resolved logging directory and debug log path
- selected environment variables such as `HOME`, `SHELL`, and `NO_COLOR`

## PATH AND OVERLAY CHECKS

For each reported path, Acton shows whether it exists, whether it appears
writable, and when relevant which resolution source selected it.

Overlay inspection includes:

- whether local and global overlay files parse correctly
- how many entries they contain
- a merged entry count when both layers load successfully

## STDLIB HEALTH

The stdlib section reports whether `.acton/tolk-stdlib` is:

- `missing`
- `incomplete`
- `healthy`
- `outdated`
- `unknown-version`

## EXIT STATUS

- `0`: The diagnostic report was produced successfully.
- `1`: Environment inspection failed before the report could be completed, for
  example because path resolution or manifest loading failed unexpectedly.

## EXAMPLES

1. Show diagnostics for the current project:

   ```bash
   acton doctor
   ```

2. Inspect another project root explicitly:

   ```bash
   acton --project-root ../my-project doctor
   ```

3. Capture environment details before filing a bug:

   ```bash
   acton doctor
   ```

## SEE ALSO

- `acton help init`
- `acton help ls`
- [Acton documentation](https://ton-blockchain.github.io/acton/docs/welcome)
