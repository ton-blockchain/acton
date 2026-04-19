# acton-up(1)

## Name

acton-up --- Update the Acton binary from published releases

## Synopsis

`acton up` [_options_] [_version_]

## Description

Download and install a published Acton release for the current target triple.

By default, stable builds update to the latest stable release. Trunk builds
stay on the trunk channel unless `--stable` is passed explicitly.

The updater fetches release metadata from GitHub, downloads the matching
archive and SHA256 checksum, verifies the checksum, and replaces the current
binary.

## Options

### Update Options

{{#options}}

{{#option "_version_" }}
Specific version tag to install.
{{/option}}

{{#option "`--trunk`" }}
Install the most recent trunk release.
{{/option}}

{{#option "`--stable`" }}
Install the latest stable release.
{{/option}}

{{#option "`--force`" }}
Install the selected release even if Acton is already up to date.
{{/option}}

{{#option "`-y`, `--yes`" }}
Skip confirmation prompts.
{{/option}}

{{#option "`--list`" }}
List available release tags and exit.
{{/option}}

{{/options}}

### Display Options

{{> options-display }}

### Project Options

{{> options-project-pass-through }}

## Release Sources

Acton queries GitHub release metadata and assets from:

- `ton-blockchain/acton`
- `i582/acton-public`

If `GITHUB_TOKEN` is set, Acton uses it for authenticated GitHub API requests.

## Installation Flow

When installing a release, Acton:

- selects the archive matching the current target triple
- downloads the archive and its `.sha256` file
- verifies the SHA256 digest
- unpacks the archive
- replaces the current `acton` binary

`acton up --list` is read-only and skips download or replacement.

## Homebrew Installations

If the current binary path looks like a Homebrew installation, Acton warns and
recommends using:

```bash
brew upgrade acton
```

Without `--yes`, Acton asks whether to continue with the built-in updater.

## Interactivity

- `--force` bypasses the usual up-to-date short-circuit for the selected release
- `--yes` suppresses confirmation prompts for non-interactive use
- Homebrew-style installs prompt before replacement unless `--yes` is passed
- up-to-date checks and `--list` can succeed without making local changes

## Channel Selection

- `acton up` on a stable build updates to the latest stable release
- `acton up` on a trunk build stays on trunk
- `acton up --stable` switches a trunk build back to stable
- `acton up --trunk` switches to the trunk channel
- `acton up <version>` installs the explicit version even if it is older or
  newer than the current one

## Exit Status

- `0`: The requested release information was listed, Acton was already up to
  date, or the selected version was installed successfully.
- `1`: Release lookup failed, checksum verification failed, archive replacement
  failed, or the update was cancelled or could not be confirmed.

## Examples

1. Update to the latest stable release:

   ```bash
   acton up
   ```

2. Switch to the trunk channel:

   ```bash
   acton up --trunk
   ```

3. Install a specific version:

   ```bash
   acton up 0.1.0
   ```

4. List available releases:

   ```bash
   acton up --list
   ```

5. Reinstall the currently selected channel version:

   ```bash
   acton up --force
   ```

6. Update non-interactively in CI or bootstrap scripts:

   ```bash
   acton up --stable -y
   ```

## See Also

- [Installation guide](https://ton-blockchain.github.io/acton/docs/installation)
