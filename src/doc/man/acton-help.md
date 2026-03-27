# acton-help(1)

## NAME

acton-help --- Show root help or detailed help for a specific top-level command

## SYNOPSIS

`acton help` [_options_] [_command_]

## DESCRIPTION

Show help for the Acton CLI.

Without an argument, `acton help` prints the root command overview.

For detailed manuals, prefer `acton help <command>` for top-level commands.
Generated manuals document behavior, configuration, side effects, and examples
in more detail than short flag help.

With a command name, Acton first tries to render a generated command manual
from the bundled `txt` and `man` artifacts. If no generated manual exists,
Acton falls back to clap long help for that subcommand.

Nested command paths such as `wallet list` are not currently accepted by
`acton help`. In practice, top-level manuals such as `acton help wallet` and
`acton help library` already include the nested subcommands under that command.
Use clap help directly only when you need the exact nested help view, for
example `acton wallet list --help`.

If the command name is unknown, Acton reports an error and may suggest a
similar command name.

## OPTIONS

### Help Options

{{#options}}

{{#option "_command_" }}
Top-level command to show help for.

Top-level manuals usually include summaries of nested subcommands.

Nested command paths are not currently supported.
{{/option}}

{{/options}}

### Display Options

{{> options-display }}

### Project Options

{{> options-project-pass-through }}

## PAGER AND FALLBACK

- when output is a terminal, `acton help <command>` prefers rendering through
  `man`, then `less`, then `more`
- when output is not a terminal, Acton writes the generated plain-text manual
  directly to standard output
- piping or redirecting output disables pager rendering automatically
- when no generated manual exists for a top-level command, Acton falls back to
  clap long help, which is typically shorter and flag-focused
- `acton help` itself does not require a project to exist

## EXIT STATUS

- `0`: Help was printed successfully.
- `1`: The requested command was unknown or help rendering failed.

## EXAMPLES

1. Show the root command overview:

   ```bash
   acton help
   ```

2. Show the generated manual for `build`:

   ```bash
   acton help build
   ```

3. Inspect nested wallet subcommands through the top-level manual:

   ```bash
   acton help wallet
   ```

4. Show the exact clap help for a nested subcommand:

   ```bash
   acton wallet list --help
   ```

5. Show help for an unknown command:

   ```bash
   acton help not-a-command
   ```

6. Search within a manual without invoking the pager:

   ```bash
   acton help build | rg "EXIT STATUS|EXAMPLES"
   ```

## SEE ALSO

- `acton --help`
- `acton help new`
- [Commands overview](https://ton-blockchain.github.io/acton/docs/commands)
