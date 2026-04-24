# Toolchain Design

Status: draft

This document describes how Acton should select, install, and run the Acton
version required by a project.

## Goals

- Let projects pin the toolchain in `Acton.toml`.
- Let users override the project toolchain for one command with Cargo-style
  `+<version>` syntax.
- Treat the Tolk version as the primary project toolchain version.
- Treat the Acton version as the concrete executable version that ships one
  bundled Tolk version.
- Avoid a separate shim binary. Any installed `acton` binary must be able to
  resolve the project toolchain and re-execute another installed Acton binary
  when needed.
- Ask before installing a missing Acton version in interactive terminals.
- Keep project toolchain installs side-by-side instead of replacing the user's
  global `acton` binary.
- Reuse the existing `acton up` release lookup, download, checksum, and archive
  extraction logic wherever possible.

## Non-Goals

- Do not support multiple Tolk versions inside a single Acton release.
- Do not make `Acton.toml` a package manager lockfile.
- Do not install a project-local shim into the repository.
- Do not make the project toolchain resolver depend on shell-specific behavior.
- Do not change Tolk source-level `tolk <version>` semantics in this design.
- Do not add a global `--yes` flag for toolchain installation.
- Do not auto-install missing toolchains during normal non-interactive project
  commands. CI should use an explicit `acton toolchain install` preinstall step.
- Do not allow `trunk` as a project toolchain. Trunk remains only a global
  `acton up --trunk` channel.

## Configuration

Projects can add a top-level `[toolchain]` section:

```toml
[toolchain]
acton = "0.3.0"
tolk = "1.3.0"
```

Both fields are optional, but at least one field must be present for Acton to
enable project toolchain resolution.

### Fields

| Field   | Meaning                                    | Initial rule |
|:--------|:-------------------------------------------|:-------------|
| `acton` | Exact Acton release version to execute.    | Semver, no ranges. A leading `v` may be accepted but should not be written back. |
| `tolk`  | Exact Tolk language/compiler version requested. | Semver, no ranges and no partial versions. |

`acton = "trunk"` is not accepted in project config. Trunk is useful for
`acton up --trunk`, but project toolchains should be reproducible stable
releases.

## CLI Toolchain Override

Acton should support a Cargo-style leading toolchain selector:

```bash
acton +0.3.0 build
acton +0.3.0 test --filter transfer
acton +0.3.0 script scripts/deploy.tolk --net testnet
```

`+0.3.0` means "run this command with Acton/toolchain 0.3.0". The bundled Tolk
version is whatever that Acton release ships. This is equivalent to a transient
`[toolchain].acton = "0.3.0"` for one invocation.

The selector:

- must be the first argument after `acton`, before global flags and the
  subcommand;
- must be an exact Acton/toolchain semver version;
- is not written to `Acton.toml`;
- overrides the entire `[toolchain]` section for this invocation;
- follows the same install, prompt, and re-exec flow as `[toolchain].acton`.

Examples:

```bash
# Ignore the project's pinned toolchain for this run.
acton +0.3.0 test

# Works from any subdirectory using the normal project-root resolution rules.
acton +0.3.0 --manifest-path ../Acton.toml build
```

`+trunk`, partial versions such as `+0.3`, and Tolk selectors such as
`+tolk:1.3.0` are out of scope for the first implementation.

## Resolution Rules

Acton resolves a project toolchain before executing project-aware commands.

Explicit one-command overrides have the highest priority:

1. `+<acton-version>` CLI selector.
2. `[toolchain]` from `Acton.toml`.
3. Current binary when no toolchain is requested.

### No `[toolchain]`

Run the current binary unless a `+<acton-version>` selector was provided.

### Only `acton`

Use the requested Acton version. The bundled Tolk version is whatever that
Acton release ships.

If that Acton version is not installed in the side-by-side toolchain store,
prompt to install it.

### Only `tolk`

If the currently running Acton binary ships the exact requested Tolk version,
continue with the current binary.

Otherwise, choose the newest stable Acton release that ships the exact Tolk
version and is not yanked. If that Acton version is not installed in the
side-by-side toolchain store, prompt to install it.

### Both `acton` and `tolk`

Use the requested Acton version only if its bundled Tolk version exactly matches
the requested Tolk version.

If the versions conflict, fail before running the command:

```text
Error: Acton.toml requests acton 0.3.0 and tolk 1.3.0, but acton 0.3.0 ships tolk 1.2.0.
```

The error should suggest either removing the `acton` pin or changing it to one
of the known Acton versions that ship the requested Tolk version.

## Command Scope

Toolchain resolution should run for project-aware commands such as:

- `build`
- `check`
- `compile` when a project manifest is active
- `fmt`
- `run`
- `script`
- `test`
- `verify`
- `wrapper`

Toolchain resolution should not install or re-execute for commands whose job is
to inspect or manage Acton itself:

- `acton --version`
- `acton help`
- `acton up`
- future `acton toolchain ...` management commands
- shell completion generation
- `acton new`
- `acton init` before a manifest exists

If a `+<acton-version>` selector is provided for a command where toolchain
resolution is intentionally skipped, Acton should fail with a short message
that the selector is only supported for project toolchain commands.

For skipped commands, Acton may still parse `Acton.toml` when the command
already does that today, but it must not auto-install or re-execute.

## Version Metadata

The resolver needs a reliable mapping from Acton releases to bundled Tolk
versions:

```text
acton version -> bundled tolk version
tolk version  -> newest acton version that ships that exact tolk version
```

Each Acton binary should know the exact Tolk version it ships. This should be
build-time metadata exposed through `build_info`, not a separate per-release
lookup:

```text
current acton binary -> bundled tolk version
```

For selecting not-yet-installed versions, publish an aggregate toolchain index:

```json
{
  "schema": 1,
  "generated_at": "2026-04-24T00:00:00Z",
  "releases": [
    {
      "acton": "0.3.0",
      "tolk": "1.3.0",
      "tag": "v0.3.0",
      "stable": true,
      "yanked": true,
      "yank_reason": "broken wrapper generation for generic storage types"
    },
    {
      "acton": "0.3.1",
      "tolk": "1.3.0",
      "tag": "v0.3.1",
      "stable": true,
      "yanked": false
    },
    {
      "acton": "0.3.2",
      "tolk": "1.3.0",
      "tag": "v0.3.2",
      "stable": true,
      "yanked": false
    }
  ]
}
```

The index is the source of truth for remote resolution. It tells Acton which
stable releases are still supported install candidates for each exact Tolk
version. A release can still validate itself from its embedded build metadata
after installation.

`yanked = true` means the release should not be selected by automatic
resolution. Yanked releases stay in the index so Acton can explain existing
pins and show why a version is no longer recommended.

Resolution rules for yanked releases:

- `tolk = "..."` and `acton toolchain install` without a version choose the
  newest stable non-yanked Acton release for that exact Tolk version.
- `[toolchain].acton = "..."`, `+<acton-version>`, and
  `acton toolchain install <acton-version>` fail if the requested version is
  yanked.
- If local metadata says an already-installed version is yanked, project
  commands should still fail before re-exec. Running known-yanked toolchains
  silently would make yanks ineffective.
- If an explicit Acton version is already installed and no toolchain index or
  cached yank metadata is available, Acton may run it. Yank enforcement depends
  on the best available index/cache/local metadata, not on mandatory fresh
  network access.
- `yank_reason` is optional, but when present it should be shown in diagnostics.

### Index source and caching

The aggregate index lives as a JSON file in the repository root. Proposed name:

```text
toolchain-index.json
```

Fetch order mirrors `acton up` release lookup:

1. `i582/acton-public`
2. `ton-blockchain/acton`

Acton should fetch the file from the default branch using the GitHub contents or
raw file API. If the file is missing from the first repository, try the next
repository. If fetching fails because of network/API errors, fall back to the
local cache when possible.

Cache layout:

```text
~/.acton/
  toolchains/
    index.json
    index-meta.json
```

`index.json` stores the last valid index. `index-meta.json` records at least:

```json
{
  "fetched_at": "2026-04-24T12:00:00Z",
  "source_repo": "i582/acton-public",
  "source_ref": "main",
  "etag": "\"abc123\""
}
```

Caching rules:

- Parse and validate a fresh index before replacing the cached index.
- Use conditional requests with `ETag` or `If-Modified-Since` when GitHub
  provides validators.
- A fresh fetch should be attempted when the cache is older than 24 hours.
- If the cache is older than 24 hours and refresh fails, use the stale cache for
  project command resolution and show a warning only for explicit management
  commands such as `acton toolchain list`, `install`, and `resolve`.
- If there is no cache and the index cannot be fetched, commands that need
  remote resolution fail with the index-unavailable diagnostic.
- If an explicit target Acton version is already installed, Acton should not
  need fresh network access to run it.
- Yank enforcement uses the best available local knowledge: cached index data,
  installed `release.json` metadata if it records yank status, or a fresh index
  if one is available.
- If no index/cache/local yank metadata is available for an already-installed
  explicit Acton version, Acton may run it instead of requiring network access
  just to prove that it is not yanked.

### Index maintenance in `xtask`

The index should be updated by release automation, not by hand-editing during a
release.

Add an `xtask` module for `toolchain-index` operations:

```text
cargo xtask toolchain-index check
cargo xtask toolchain-index add-release --version 0.3.2
cargo xtask toolchain-index yank --version 0.3.0 --reason "broken wrapper generation for generic storage types"
```

`cargo xtask release --version X.Y.Z` should run the equivalent of
`toolchain-index add-release --version X.Y.Z` after bumping project versions and
before creating the release commit. The release commit should therefore include
the updated `toolchain-index.json` together with `Acton.toml`, `Cargo.toml`,
`Cargo.lock`, and `package.json`.

`add-release` behavior:

- Read the Acton version from `--version`.
- Read the bundled Tolk version from
  `workspace.metadata.acton.tolk-version` in the root `Cargo.toml`, which is
  the same canonical source used by the build script to expose
  `build_info::TOLK_VERSION` and print `acton -V`.
- Add or update one entry:

  ```json
  {
    "acton": "0.3.2",
    "tolk": "1.3.0",
    "tag": "v0.3.2",
    "stable": true,
    "yanked": false
  }
  ```

- Set `generated_at` to the current UTC timestamp.
- Keep existing yanked entries unchanged.
- Sort releases by Acton semver ascending.
- Fail if the target version already exists with a different Tolk version.
- Fail if the Tolk version is missing or is not exact semver.
- Run `toolchain-index check` after writing.

`check` behavior:

- Validate JSON syntax and schema version.
- Validate exact semver for `acton` and `tolk`.
- Validate `tag == "v{acton}"`.
- Validate each Acton version appears once.
- Validate `yank_reason` is present and non-empty when `yanked = true`.
- Validate yanked entries remain visible in the index.
- Validate releases are sorted by Acton semver.

`yank` behavior:

- Mark an existing release as `yanked = true`.
- Write or replace `yank_reason`.
- Update `generated_at`.
- Refuse to yank an unknown Acton version.
- Run `toolchain-index check` after writing.

Publishing requirements:

- The source repo keeps `toolchain-index.json` at the repository root.
- The public mirror `i582/acton-public` must also have the same root
  `toolchain-index.json`, because Acton fetches that repository first.
- The release workflow or mirror workflow should copy the updated index to
  `acton-public` before or together with public release publication.
- If the public mirror update fails, the fallback to `ton-blockchain/acton`
  still works, but release automation should treat the mirror failure as a
  release issue to fix.

## Install Layout

Project toolchains should be installed side-by-side under the existing Acton
home, not inside the user's project.

Proposed layout:

```text
~/.acton/
  toolchains/
    index.json
    0.3.0/
      acton
      release.json
    0.3.1/
      acton
      release.json
  downloads/
    ...
```

The executable path for a resolved toolchain is:

```text
~/.acton/toolchains/<acton-version>/acton
```

`release.json` records the target triple that was installed, the Acton/Tolk
versions reported by the binary after install, and any known yank metadata from
the index used during installation. The install path does not include the target
triple because one Acton home is expected to belong to one machine/platform. If
that assumption changes later, the install layout can grow a target-triple level
without changing `Acton.toml`.

`acton up` continues to replace the global binary in `PATH`. Project toolchain
installation uses shared lower-level release lookup, download, checksum, and
archive extraction code, but writes into the side-by-side toolchain store.

## `acton up`

`acton up` remains the global self-updater. It is intentionally separate from
project toolchain installation.

Behavior:

- `acton up` updates or replaces the currently running global Acton binary in
  `PATH`, preserving the current `acton up` mental model.
- `acton up <version>` installs that version as the global Acton binary, not as
  a project side-by-side toolchain.
- `acton up --trunk` remains a global trunk-channel operation.
- `acton up` does not read `[toolchain]`, does not resolve project Tolk
  versions, and does not re-exec into project toolchains.
- `acton +0.3.0 up` is invalid because `up` is not a project toolchain command.
- `acton up --list` lists global Acton release tags. Toolchain compatibility
  information belongs to `acton toolchain list`.
- The implementation should reuse the same release client, checksum
  verification, download, and archive extraction primitives as
  `acton toolchain install`, but the final install destination is different.

User guidance:

```text
Use `acton up` when you want to change the Acton binary found in PATH.
Use `acton toolchain install` when you want to install the version required by a project.
```

## Startup Flow

Every Acton binary follows this startup flow:

1. Parse enough CLI state to know global path flags, the subcommand, and whether
   toolchain resolution is allowed for this invocation. This starts by stripping
   an optional `argv[1]` `+<acton-version>` selector before clap parsing.
2. Resolve the project root and manifest path using the same rules documented
   for `Acton.toml`.
3. Load `[toolchain]` if present and no `+<acton-version>` selector was provided.
4. Resolve the target Acton version.
5. If the target is the current binary version, continue normally.
6. If the target binary is installed, re-execute it with the original argv and
   relevant environment.
7. If the target binary is missing and the terminal is interactive, ask for
   confirmation and install it before re-executing.
8. If the target binary is missing and the terminal is non-interactive, fail
   with a command the user can run explicitly.

On Unix, use `exec` so the final Acton process owns the original process ID and
signal behavior. On unsupported platforms, spawn the selected binary, wait for
it, and return its exit code.

## Re-Exec Guard

The resolver must prevent loops. Before re-executing, set internal environment
variables:

```text
ACTON_TOOLCHAIN_REQUESTED_ACTON=0.3.0
ACTON_TOOLCHAIN_PARENT_ACTON=0.3.1
ACTON_TOOLCHAIN_REEXEC_DEPTH=1
ACTON_TOOLCHAIN_SOURCE=cli-plus
```

When the target was resolved from `[toolchain].tolk`, also set:

```text
ACTON_TOOLCHAIN_REQUESTED_TOLK=1.3.0
```

The child process checks:

- if its own Acton version matches `ACTON_TOOLCHAIN_REQUESTED_ACTON`, continue;
- if `ACTON_TOOLCHAIN_REQUESTED_TOLK` is set, its bundled Tolk version must also
  match;
- if it does not match, fail with an internal toolchain error instead of trying
  to install another version indefinitely;
- if depth is greater than a small fixed limit, fail.

These variables are internal and should not be documented as user API.

## Install Prompt

When a required version is missing and stdin is interactive, prompt with
`inquire::Confirm`:

```text
Project requires acton 0.3.0 (Tolk 1.3.0). Install it now? (Y/n)
```

If the user declines, exit with status `1`.

For CI and other non-interactive environments, do not prompt. Fail with a clear
message:

```text
Error: Project requires acton 0.3.0 (Tolk 1.3.0), but it is not installed.
Run `acton toolchain install` from the project root or `acton toolchain install 0.3.0`.
```

For CI, use explicit preinstall:

```bash
acton toolchain install
acton test

acton toolchain install 0.3.0
acton test
```

`acton toolchain install` is explicit consent to install the resolved project
toolchain, so it must not ask the same install confirmation again.

## Concurrency and Atomicity

Toolchain installation must be safe when several terminals or CI jobs start at
the same time.

- Use a lock file per target version.
- Download into a temporary directory under `~/.acton/downloads`.
- Verify SHA256 before unpacking.
- Unpack into a temporary install directory.
- Write `release.json` after verification.
- Rename the temporary install directory into the final path atomically.
- If another process installed the same version while waiting for the lock,
  reuse the installed result.

## Security

- Keep using release asset SHA256 verification.
- Execute binaries only from the canonical Acton toolchain store.
- Never execute binaries from the project directory.
- Reject path separators and non-semver values in `[toolchain].acton`.
- Cache toolchain index metadata, but treat the downloaded binary checksum as the final
  integrity check.
- Record the installed target triple, Acton/Tolk versions, and known yank
  metadata in `release.json`.

Future hardening can add signed toolchain index metadata, but checksum verification
should be enough for the first iteration because it matches the current
`acton up` trust model.

## UX Commands

Add a dedicated command group for side-by-side project toolchains:

```text
acton toolchain list
acton toolchain install [acton-version]
acton toolchain remove <acton-version>
acton toolchain which
acton toolchain resolve
```

Initial behavior:

- `list` shows installed toolchains and known remote mappings.
- `install <acton-version>` installs the exact Acton release into the
  side-by-side store.
- `install` without a version resolves the current project's `[toolchain]` and
  installs the selected Acton release. This is the recommended CI preinstall
  command.
- `install` without a version uses the normal project root and manifest
  resolution rules, including `--project-root` and `--manifest-path`.
- `install` succeeds as a no-op if the resolved toolchain is already installed
  or the current binary already satisfies the requested toolchain.
- `install` without a version fails if there is no project manifest or the
  project has no `[toolchain]` section.
- `remove` deletes an installed side-by-side version after confirmation.
- `which` prints the executable path selected for the current project.
- `resolve` prints machine-readable resolution details for debugging and CI.

## User-Facing Diagnostics

Toolchain errors should be direct and actionable. Every user-facing diagnostic
should try to include:

- what was requested;
- where it came from: `[toolchain]`, `+<acton-version>`, or `acton toolchain install`;
- what Acton resolved or tried to resolve;
- whether the command can be fixed by editing `Acton.toml`, running
  `acton toolchain install`, or choosing another version.

### Invalid project configuration

If `[toolchain]` exists but neither `acton` nor `tolk` is set, fail while
loading config:

```text
Error: [toolchain] in Acton.toml is empty.
Set `acton = "0.3.0"` or `tolk = "1.3.0"`, or remove the section.
```

If `[toolchain].acton` is not an exact Acton semver version, fail:

```text
Error: [toolchain].acton must be an exact Acton version, got "0.3".
Use a full version such as `acton = "0.3.0"`.
```

If `[toolchain].acton` contains `trunk`, a range, path separators, or any
non-version syntax, fail:

```text
Error: [toolchain].acton must be a stable Acton release version, got "trunk".
Project toolchains do not support trunk builds.
```

If `[toolchain].tolk` is not an exact Tolk semver version, fail:

```text
Error: [toolchain].tolk must be an exact Tolk version, got "1.3".
Use a full version such as `tolk = "1.3.0"`.
```

### Conflicting project pins

If both fields are set and the requested Acton release does not ship the
requested Tolk version, fail before command execution:

```text
Error: Acton.toml requests acton 0.3.0 and tolk 1.3.0, but acton 0.3.0 ships tolk 1.2.0.
Choose one fix:
  1. Change `[toolchain].acton` to an Acton release that ships Tolk 1.3.0.
  2. Remove `[toolchain].acton` and keep `tolk = "1.3.0"` so Acton can choose the newest supported release.
Then run `acton toolchain install`.
```

When the index is available, include a short suggestion:

```text
Known Acton releases for Tolk 1.3.0: 0.3.1, 0.3.2
Recommended fix:
  [toolchain]
  acton = "0.3.2"
  tolk = "1.3.0"

Or let Acton choose the newest supported release:
  [toolchain]
  tolk = "1.3.0"

After editing Acton.toml, run `acton toolchain install`.
```

### CLI selector errors

If `+<acton-version>` is partial or invalid, fail before clap command dispatch:

```text
Error: `+` expects an exact Acton/toolchain version, got `+0.3`.
Use a full version such as `acton +0.3.0 test`.
```

If the selector is used anywhere except `argv[1]`, treat it as an ordinary
argument for the target command. We should not support multiple positions in
the first implementation.

If `+<acton-version>` is used with a command where toolchain resolution is
skipped, fail:

```text
Error: `+0.3.0` can only be used with project toolchain commands.
Run `acton up 0.3.0` to change the global Acton binary, or run a project command such as `acton +0.3.0 test`.
```

### Unknown versions

If an explicit Acton version is not in the toolchain index, fail:

```text
Error: Acton 0.3.0 is not listed in the toolchain index.
Run `acton toolchain list` to see known versions.
```

If a Tolk version has no supported Acton release in the toolchain index, fail:

```text
Error: No supported Acton release ships Tolk 1.3.0.
Run `acton toolchain list` to see known Tolk versions.
```

If an explicit Acton version is yanked, fail:

```text
Error: Acton 0.3.0 has been yanked and cannot be selected.
Reason: broken wrapper generation for generic storage types.
Use Acton 0.3.2 instead, or remove `[toolchain].acton` and keep `tolk = "1.3.0"`.
```

If the index cannot be fetched but the requested Acton version is already
installed locally, continue. If the target is missing and the index is required,
fail:

```text
Error: Could not resolve Tolk 1.3.0 because the toolchain index is unavailable.
Check your network connection, or preinstall an explicit Acton version with `acton toolchain install 0.3.0`.
```

### Missing toolchain

If the resolved Acton version is missing and the terminal is interactive, prompt:

```text
Project requires acton 0.3.0 (Tolk 1.3.0). Install it now? (Y/n)
```

If the user declines, fail:

```text
Error: Acton 0.3.0 is required but is not installed.
Run `acton toolchain install` from the project root or `acton toolchain install 0.3.0`.
```

If the resolved Acton version is missing and the terminal is non-interactive,
fail without prompting:

```text
Error: Project requires acton 0.3.0 (Tolk 1.3.0), but it is not installed.
Run `acton toolchain install` from the project root or `acton toolchain install 0.3.0`.
```

### `acton toolchain install`

If `acton toolchain install` is run without a version outside a project, fail:

```text
Error: No Acton.toml found, so there is no project toolchain to install.
Run `acton toolchain install 0.3.0` to install an explicit Acton version.
```

If it is run in a project with no `[toolchain]`, fail:

```text
Error: Acton.toml has no [toolchain] section.
Add `[toolchain]` or run `acton toolchain install 0.3.0`.
```

If the resolved toolchain is already installed, succeed as a no-op:

```text
Acton 0.3.0 is already installed.
```

If the current binary already satisfies the requested project toolchain, also
succeed as a no-op:

```text
Current Acton 0.3.0 already satisfies this project toolchain.
```

### Download and install failures

If the release does not contain an asset for the current target, fail:

```text
Error: Acton 0.3.0 does not provide a binary for aarch64-apple-darwin.
Run `acton toolchain list` to inspect available releases.
```

If download fails, keep the message network-specific:

```text
Error: Failed to download Acton 0.3.0.
Check your network connection and try `acton toolchain install 0.3.0` again.
```

If checksum verification fails, delete temporary files and fail:

```text
Error: Downloaded Acton 0.3.0 failed SHA256 verification.
Acton removed the temporary download files from ~/.acton/downloads.
Try `acton toolchain install 0.3.0` again later.
```

If archive extraction fails, delete temporary files and fail:

```text
Error: Failed to unpack Acton 0.3.0.
Acton removed the temporary install directory.
Try `acton toolchain install 0.3.0` again.
```

If writing to the toolchain store fails, report the path:

```text
Error: Failed to install Acton 0.3.0 into ~/.acton/toolchains/0.3.0.
Check directory permissions and available disk space.
```

If another process is installing the same version, wait on the lock. If the lock
cannot be acquired or times out, fail:

```text
Error: Another Acton process is installing 0.3.0 and the install lock timed out.
Try again after the other process exits.
```

### Corrupt installed toolchains

After installation, Acton should run the selected binary with an internal
version probe before re-exec. If the binary reports the wrong Acton version,
fail:

```text
Error: Installed toolchain at ~/.acton/toolchains/0.3.0/acton reports Acton 0.3.1.
Run `acton toolchain remove 0.3.0` and then `acton toolchain install 0.3.0`.
```

If the binary was selected because of a Tolk pin but reports a different Tolk
version, fail:

```text
Error: Acton 0.3.0 was selected for Tolk 1.3.0, but it reports Tolk 1.2.0.
Update the toolchain index or reinstall the toolchain.
```

### Re-exec failures

If the child binary cannot be executed, fail with the selected path:

```text
Error: Failed to execute ~/.acton/toolchains/0.3.0/acton.
Check file permissions or reinstall with `acton toolchain install 0.3.0`.
```

If the re-exec guard detects a loop or a version mismatch, fail as an internal
toolchain error:

```text
Error: Toolchain re-exec failed: requested Acton 0.3.0, but child process is Acton 0.3.1.
Run `acton toolchain remove 0.3.0` and reinstall it.
```

### Older Acton versions

An Acton version released before this feature cannot diagnose `[toolchain]`
because it will not know about the section. The release notes and docs for the
first supported version must say that projects using `[toolchain]` require at
least that Acton version as the bootstrap binary.

## Implementation Plan

1. Add `ToolchainConfig` to `acton-config` and update the JSON schema.
2. Add `workspace.metadata.acton.tolk-version` in the root `Cargo.toml` as
   the build-time source for `TOLK_VERSION` metadata exposed through
   `build_info` and `acton -V`.
3. Add pre-clap parsing for a leading `+<acton-version>` selector.
4. Extract reusable release download/install primitives from `acton up`.
5. Add toolchain index structs and resolver logic.
6. Add side-by-side install layout, locking, and atomic install.
7. Add early startup resolution and re-exec before command dispatch.
8. Add `acton toolchain` commands.
9. Add `cargo xtask toolchain-index` commands and wire `add-release` into
   `cargo xtask release`.
10. Document `[toolchain]` and `+<acton-version>` in docs after the
   behavior is implemented.

## Testing Strategy

Tests should cover resolver behavior separately from command-line UX, but every
user-facing message should still have snapshot coverage. Integration tests
should prefer snapshots over plain string asserts.

### Test fixtures

Use deterministic fixtures instead of real GitHub/network access:

- temporary Acton home with isolated `toolchains/`, `downloads/`, and cache
  files;
- synthetic `Acton.toml` projects;
- synthetic toolchain indexes with stable, yanked, unknown, and malformed
  releases;
- fake release client that records which repository was queried first;
- fake archives/checksum files for install tests;
- fake installed `acton` binaries that can report configured Acton/Tolk
  versions and record argv/env for re-exec tests.

### Config parsing and schema

Unit tests:

- `[toolchain]` absent: config loads and resolver sees no project request.
- `[toolchain]` empty: error points to empty section.
- `acton = "0.3.0"` accepted.
- `acton = "v0.3.0"` accepted or normalized if implementation chooses to
  support leading `v`.
- `acton = "0.3"` rejected as partial.
- `acton = ">=0.3.0"`, `acton = "trunk"`, and values with path separators are
  rejected.
- `tolk = "1.3.0"` accepted.
- `tolk = "1.3"` rejected as partial.
- both `acton` and `tolk` set: config parses; compatibility is resolver's job.
- JSON schema exposes `[toolchain].acton` and `[toolchain].tolk`.

Snapshot tests:

- invalid `acton`;
- invalid `tolk`;
- empty `[toolchain]`;
- trunk in project config.

### CLI selector parsing

Unit tests:

- `acton +0.3.0 test` extracts selector `0.3.0` and leaves argv as
  `acton test`.
- selector must be `argv[1]`; `acton test +0.3.0` is not treated as a
  toolchain selector.
- `+0.3`, `+trunk`, `+tolk:1.3.0`, and non-semver values are rejected before
  command dispatch.
- selector overrides the whole `[toolchain]` section.
- `acton +0.3.0 up`, `help`, `toolchain`, and shell completion flows fail
  because they are skipped commands.

Snapshot tests:

- invalid selector;
- selector with skipped command;
- selector overriding conflicting project config.

### Resolver matrix

Unit tests with synthetic indexes:

- no `[toolchain]` and no `+`: current binary is selected.
- only `acton`, same as current binary: no install or re-exec.
- only `acton`, installed side-by-side: select installed path.
- only `acton`, missing: return install-required result.
- only `acton`, yanked: fail with yank reason.
- only `acton`, unknown: fail with known-versions hint.
- only `tolk`, current binary ships exact Tolk: current binary is selected.
- only `tolk`, current binary does not match: choose newest stable non-yanked
  Acton release that ships exact Tolk.
- only `tolk`, newest matching release is yanked but older non-yanked exists:
  choose older non-yanked release.
- only `tolk`, all matching releases are yanked: fail with no supported release.
- only `tolk`, no matching release: fail with known-Tolk hint.
- both `acton` and `tolk` match: selected Acton version wins.
- both set but Acton ships a different Tolk: fail with concrete suggested
  `Acton.toml` edits when index is available.
- `+<acton-version>` with project `[toolchain]`: selected Acton version comes
  from `+`, project config is ignored.

Snapshot tests:

- conflict with suggested replacement;
- unknown Acton;
- unknown Tolk;
- yanked exact Acton;
- Tolk-only resolution skipping yanked releases.

### Index source and cache

Unit tests:

- fetch order tries `i582/acton-public` before `ton-blockchain/acton`.
- if the file is missing in `acton-public`, fallback tries `ton-blockchain/acton`.
- successful fresh fetch writes `index.json` and `index-meta.json`.
- invalid fresh index does not replace existing cache.
- `ETag` or `If-Modified-Since` validators are sent when present in metadata.
- cache younger than 24 hours is used without fresh fetch where allowed.
- cache older than 24 hours triggers refresh.
- stale cache plus failed refresh is still usable for project command
  resolution.
- stale cache plus failed refresh emits warnings for `toolchain list`,
  `install`, and `resolve`.
- no cache plus failed fetch fails for any resolution that needs remote index.
- installed exact Acton can run without fresh network access when no cache is
  available.
- installed exact Acton is rejected when cached index or installed
  `release.json` metadata marks it yanked.

Snapshot tests:

- index unavailable with no cache;
- stale cache warning on management command;
- invalid index diagnostic.

### `acton toolchain install`

Integration tests with fake release client and temp Acton home:

- `acton toolchain install 0.3.0` installs exact Acton version.
- `acton toolchain install` from a project with `[toolchain].acton` installs
  that exact version.
- `acton toolchain install` from a project with only `[toolchain].tolk` installs
  newest stable non-yanked compatible Acton.
- no project manifest and no explicit version fails.
- project with no `[toolchain]` and no explicit version fails.
- already installed version succeeds as no-op.
- current binary already satisfies project toolchain succeeds as no-op.
- explicit yanked version fails and does not install.
- missing target asset fails.
- download failure leaves no final install directory.
- checksum mismatch removes temporary downloads.
- archive extraction failure removes temporary install directory.
- permission/disk failure reports target install path.
- lock contention waits and reuses the installed result when another process
  finishes first.
- lock timeout fails with lock diagnostic.
- concurrent installs for the same version produce one final directory.
- final install is atomic: no partially populated final directory is observed.

Snapshot tests:

- no project for versionless install;
- no `[toolchain]` for versionless install;
- no-op installed;
- no-op current binary satisfies project;
- checksum mismatch;
- lock timeout.

### Project command startup and install prompt

Integration tests:

- project command with installed resolved toolchain re-execs into installed
  binary.
- project command with missing resolved toolchain in interactive mode prompts.
- accepting prompt installs and re-execs.
- declining prompt exits with status `1`.
- non-interactive mode does not prompt and prints explicit preinstall command.
- CI flow works with explicit preinstall followed by non-interactive `acton test`.
- project command does not auto-install in non-interactive mode.

Snapshot tests:

- interactive prompt text;
- user declines prompt;
- non-interactive missing version.

### Re-exec and guard behavior

Unit or integration tests with fake child binaries:

- selected child receives original argv after selector stripping.
- selected child receives relevant environment.
- `ACTON_TOOLCHAIN_REQUESTED_ACTON`, `ACTON_TOOLCHAIN_PARENT_ACTON`,
  `ACTON_TOOLCHAIN_REEXEC_DEPTH`, and `ACTON_TOOLCHAIN_SOURCE` are set.
- `ACTON_TOOLCHAIN_REQUESTED_TOLK` is set when target came from `tolk`.
- matching child Acton/Tolk versions continue.
- child Acton mismatch fails before command work.
- child Tolk mismatch fails when requested.
- re-exec depth limit prevents loops.
- child cannot be executed: diagnostic includes selected path.
- Unix path uses `exec`; non-Unix fallback propagates child exit status.

Snapshot tests:

- Acton mismatch;
- Tolk mismatch;
- re-exec loop;
- cannot execute child.

### `acton up` separation

Integration tests:

- `acton up` does not read `[toolchain]`.
- `acton up 0.3.0` installs global binary destination, not side-by-side store.
- `acton up --trunk` remains global-only.
- `acton +0.3.0 up` fails as skipped command with selector diagnostic.
- `acton up --list` shows global release tags, not Tolk compatibility table.
- shared release primitives are exercised through existing `acton up` tests and
  new `toolchain install` tests without duplicating network behavior.

Snapshot tests:

- `+<acton-version>` with `up`;
- `up --list` remains global wording.

### `acton toolchain list`, `which`, and `resolve`

Integration tests:

- `list` shows installed versions.
- `list` shows known remote versions with Tolk version, stable/yanked status,
  and yank reason when present.
- `which` in a project prints the selected executable path.
- `which` without project toolchain reports that current binary is used.
- `resolve` prints machine-readable selected source, Acton version, Tolk
  version, yanked status, installed path, and whether install is required.
- `resolve` works without installing.
- `resolve` reports conflicts and unknown versions without side effects.

Snapshot tests:

- list with yanked version;
- which with installed project toolchain;
- resolve JSON for `acton`, `tolk`, `+<acton-version>`, conflict, and missing
  install.

### User-facing diagnostics coverage

Snapshot tests should cover every diagnostic example in this document:

- invalid project configuration;
- conflicting pins with and without index suggestions;
- invalid CLI selector;
- skipped command with selector;
- unknown Acton;
- unknown Tolk;
- yanked explicit Acton;
- index unavailable;
- missing toolchain interactive decline;
- missing toolchain non-interactive;
- versionless `toolchain install` outside a project;
- versionless `toolchain install` with no `[toolchain]`;
- missing target asset;
- download failure;
- checksum failure;
- extraction failure;
- install path write failure;
- lock timeout;
- corrupt installed Acton version;
- corrupt installed Tolk version;
- child execution failure;
- re-exec guard mismatch.

### Docs and release compatibility

Documentation tests or snapshot checks should verify:

- generated `acton.schema.json` includes `[toolchain]`;
- `docs/content/docs/acton-toml.mdx` documents exact version rules;
- install docs explain explicit CI preinstall;
- release notes mention the minimum bootstrap Acton version needed to honor
  `[toolchain]`.
