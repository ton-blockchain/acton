# Releasing Acton

This document covers maintainer workflows for numbered Acton releases.

For everyday contributor setup, builds, tests, and docs workflows, see
[CONTRIBUTING.md](CONTRIBUTING.md).

## Scope

- This file covers versioned releases such as `v0.22.0`.
- `trunk` and `release-objs` releases are maintained by GitHub Actions and are
  not part of the manual release flow below.

## `xtask` surface at a glance

The full maintainer `xtask` surface currently includes:

- `release`
- `retag`
- `dist`
- `schema`
- `sync-artifacts`
- `toolchain-index`
- `github-cleanup`
- `ubicloud-cleanup`

This document focuses on numbered release flows (`release` / `retag`).
Contributor-facing setup tasks such as `schema` and `sync-artifacts` are
documented in [CONTRIBUTING.md](CONTRIBUTING.md).

## Release command

Use the release `xtask` instead of manual version bump, commit, tag, or push
steps:

```bash
cargo xtask release --version <major.minor.patch>
```

Example:

```bash
cargo xtask release --version 0.22.0
```

## Retag command

Use the retag `xtask` when a numbered release tag needs to be moved to the
current release state by creating an empty retry commit without changing the
project version:

```bash
cargo xtask retag --version 0.22.0
```

`--version` accepts `X.Y.Z` and `retag` derives the release tag as `vX.Y.Z`.

## Prerequisites

- `gh` CLI installed and authenticated: `gh auth status`
- `yq` v4 installed
- local `master` branch with no uncommitted changes
- successful GitHub Actions build for the current `master` `HEAD`
- release notes reviewed in `CHANGELOG.md`
- `workspace.metadata.acton.tolk-version` in the root `Cargo.toml` reviewed
  and updated when the bundled Tolk compiler changes
- no unresolved release-blocking issues for the target version

## What the release xtask does

`cargo xtask release`:

- validates that the version is in `X.Y.Z` format
- verifies that `CHANGELOG.md` contains a section for `X.Y.Z`
- checks that the current branch is `master`
- checks that `origin` does not already have tag `vX.Y.Z`
- verifies the worktree is clean
- fetches `origin/master` and checks local `master` is up to date
- verifies GitHub Actions builds succeeded for the current `HEAD`
- updates versions in `Acton.toml`, `Cargo.toml`, and `package.json`
- runs `cargo update --workspace`
- updates `toolchain-index.json` with the release version and the bundled Tolk
  version from `workspace.metadata.acton.tolk-version`
- creates commit `chore(acton): bump to version \`X.Y.Z\``
- creates tag `vX.Y.Z`
- shows the created commit diff stat
- asks for explicit `yes` confirmation before pushing
- pushes `master` and `vX.Y.Z` to `origin`

## After the push

After the tag is pushed, the GitHub `Release` workflow builds release
artifacts, creates the GitHub release, and only then publishes the Docker image
and mirrored release to `i582/acton-public`.

The retag workflow creates an empty commit on top of `master`, deletes the
existing tag in `origin`, recreates the tag on that retry commit, and pushes
both `master` and the tag, so it requires explicit confirmation and only runs
when the tag already exists in `origin` and local `master` exactly matches
`origin/master`.

## Other maintainer tasks

- `cargo xtask dist archive` packages release archives and `.sha256` files used
  by CI and release automation
- other `dist` subcommands currently exist but are still placeholders/TODO
- `cargo xtask schema` regenerates `acton.schema.json`
- `cargo xtask sync-artifacts` refreshes native TON artifacts and bundled stdlib assets
- `cargo xtask toolchain-index check` validates `toolchain-index.json`
- `cargo xtask toolchain-index add-release --version X.Y.Z` adds or refreshes
  an index entry using the bundled Tolk version from
  `workspace.metadata.acton.tolk-version`
- `cargo xtask toolchain-index yank --version X.Y.Z --reason "..."`
  marks a release as yanked while keeping it visible in the index
- `cargo xtask github-cleanup` and `cargo xtask ubicloud-cleanup` prune caches;
  outside CI they default to dry-run safety mode
