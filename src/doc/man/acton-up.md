# acton-up(1)

## NAME

acton-up --- Update the Acton binary from published releases

## SYNOPSIS

`acton up` [_options_] [_version_]

## DESCRIPTION

Download and install a published Acton release for the current target triple.

By default, stable builds update to the latest stable release. Trunk builds
stay on the trunk channel unless `--stable` is passed explicitly.

The updater fetches release metadata from GitHub, downloads the matching
archive and SHA256 checksum, verifies the checksum, and replaces the current
binary.

## OPTIONS

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

## RELEASE SOURCES

Acton queries GitHub release metadata and assets from:

- `ton-blockchain/acton`
- `i582/acton-public`

If `GITHUB_TOKEN` is set, Acton uses it for authenticated GitHub API requests.

## INSTALLATION FLOW

When installing a release, Acton:

- selects the archive matching the current target triple
- downloads the archive and its `.sha256` file
- verifies the SHA256 digest
- unpacks the archive
- replaces the current `acton` binary

## HOME BREW INSTALLATIONS

If the current binary path looks like a Homebrew installation, Acton warns and
recommends using:

```bash
brew upgrade acton
```

Without `--yes`, Acton asks whether to continue with the built-in updater.

## CHANNEL SELECTION

- `acton up` on a stable build updates to the latest stable release
- `acton up` on a trunk build stays on trunk
- `acton up --stable` switches a trunk build back to stable
- `acton up --trunk` switches to the trunk channel
- `acton up <version>` installs the explicit version even if it is older or
  newer than the current one

## EXAMPLES

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

## SEE ALSO

- [Installation guide](https://ton-blockchain.github.io/acton/docs/installation)
