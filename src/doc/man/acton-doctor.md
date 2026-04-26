# acton-doctor(1)

## Name

acton-doctor --- Inspect the resolved Acton project environment

## Synopsis

`acton doctor` [_options_]

## Description

Print a diagnostic report for the current Acton environment.

The report is intended to help debug project resolution, manifest parsing,
wallet and library overlays, bundled standard-library state, native emulator
and Tolk library metadata, logging paths, and relevant environment variables.

## Display Options

{{> options-display }}

## Project Options

{{> options-project-resolved }}

## Report Sections

`acton doctor` prints:

- version and build metadata
- resolved paths for the project, manifest, `.acton`, cache, wallets, and
  libraries
- `Acton.toml` existence and parse status
- wallet and library overlay load status
- bundled stdlib health and version
- native emulator/Tolk version metadata, including the TON commit hash and date
  embedded in the linked libraries
- resolved logging directory and debug log path
- selected environment variables such as `HOME`, `SHELL`, and `NO_COLOR`

## Path And Overlay Checks

For each reported path, Acton shows whether it exists, whether it appears
writable, and when relevant which resolution source selected it.

Overlay inspection includes:

- whether local and global overlay files parse correctly
- how many entries they contain
- a merged entry count when both layers load successfully

## Stdlib Health

The stdlib section reports whether `.acton/tolk-stdlib` is:

- `missing`
- `incomplete`
- `healthy`
- `outdated`
- `unknown-version`

## Exit Status

- `0`: The diagnostic report was produced successfully.
- `1`: Environment inspection failed before the report could be completed, for
  example because path resolution or manifest loading failed unexpectedly.

## Examples

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

## See Also

- `acton help init`
- `acton help ls`
- [Acton documentation](https://ton-blockchain.github.io/acton/docs/welcome)
