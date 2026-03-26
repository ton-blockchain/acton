# acton-help(1)

## NAME

acton-help --- Show root help or detailed help for a specific command

## SYNOPSIS

`acton help` [_options_] [_command_]

## DESCRIPTION

Show help for the Acton CLI.

Without an argument, `acton help` prints the root command overview.

With a command name, Acton first tries to render a generated command manual
from the bundled `txt` and `man` artifacts. If no generated manual exists,
Acton falls back to clap long help for that subcommand.

If the command name is unknown, Acton reports an error and may suggest a
similar command name.

## OPTIONS

### Help Options

{{#options}}

{{#option "_command_" }}
Command to show help for.
{{/option}}

{{/options}}

### Display Options

{{> options-display }}

### Project Options

{{> options-project-pass-through }}

## BEHAVIOR

- when output is a terminal, `acton help <command>` prefers rendering through
  `man`, then `less`, then `more`
- when output is not a terminal, Acton writes the generated plain-text manual
  directly to standard output
- `acton help` itself does not require a project to exist

## EXAMPLES

1. Show the root command overview:

   ```bash
   acton help
   ```

2. Show the generated manual for `build`:

   ```bash
   acton help build
   ```

3. Show help for an unknown command:

   ```bash
   acton help not-a-command
   ```

## SEE ALSO

- `acton --help`
- `acton help new`
- [Commands overview](https://ton-blockchain.github.io/acton/docs/commands)
