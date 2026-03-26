# acton-hooks(1)

## NAME

acton-hooks --- Scaffold and manage project-local Git hooks

## SYNOPSIS

`acton hooks` [_options_] _command_

## DESCRIPTION

Manage project-local Git hooks through Git's `core.hooksPath` mechanism.

Acton expects hooks to live in `.githooks/` inside the resolved project root.
The `hooks` command can create the scaffold, install the Git config override,
check whether it is active, and remove it again.

## SUBCOMMANDS

### acton hooks new

Create a `.githooks/` scaffold in the resolved project root.

If `--template` is omitted, Acton opens an interactive selector.

This command only works when the resolved project root already contains a local
`.git` directory.

#### Options

{{#options}}

{{#option "`--template` _template_" }}
Hook scaffold template to create.

Possible values: `empty`, `default`
{{/option}}

{{/options}}

The `default` template creates `.githooks/pre-commit` with a starter hook that
runs:

```sh
#!/bin/sh
set -e
acton check
acton fmt --check
```

The generated hook script uses `/bin/sh`, so the default template assumes a
POSIX-like shell environment.

If `.githooks/` already exists or local hooks are already configured, the
command fails instead of overwriting existing hooks.

If local Git hooks are already configured for the repository, Acton asks you to
run `acton hooks uninstall` first.

#### Examples

```bash
acton hooks new
acton hooks new --template empty
acton hooks new --template default
```

### acton hooks install

Set the repository's local `core.hooksPath` to `.githooks`.

This only works when the resolved project root contains a local `.git`
directory and `.githooks/` already exists.

Internally, this is equivalent to:

```bash
git config --local core.hooksPath .githooks
```

If `core.hooksPath` is already set locally for the repository, including to an
equivalent path like `./.githooks`, the command fails and asks you to uninstall
the existing override first.

If `.githooks/` does not exist yet, run `acton hooks new` before installing.

On success, Acton reports that Git hooks were installed from `.githooks`.

### acton hooks status

Check whether the repository's local Git config already points `core.hooksPath`
to `.githooks`.

Equivalent local paths that resolve to the same hooks directory count as
installed.

`status` checks the repository's local Git config only. Global or system
`core.hooksPath` values do not count as installed for the current project.

### acton hooks uninstall

Remove the repository-local `core.hooksPath` override.

Internally, this is equivalent to:

```bash
git config --local --unset-all core.hooksPath
```

If no local override is configured, the command succeeds and reports that hooks
are not installed.

On success, Acton reports that Git hooks were uninstalled.

## EXIT STATUS

- `0`: The selected hooks subcommand completed successfully, including no-op
  uninstall when no local override is present.
- `1`: The project was not a local Git repository, hooks were already
  configured in an incompatible way, or Git config or filesystem updates failed.

## DISPLAY OPTIONS

{{> options-display }}

## PROJECT OPTIONS

{{> options-project-resolved }}

## RECOMMENDED WORKFLOW

```bash
acton hooks new --template default
acton hooks install
acton hooks status
```

To refresh hooks after manually editing `.githooks/`, rerun:

```bash
acton hooks uninstall
acton hooks install
```

## SEE ALSO

- `acton help new`
- [Hooks command guide](https://ton-blockchain.github.io/acton/docs/commands/hooks)
