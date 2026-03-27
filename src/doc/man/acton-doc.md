# acton-doc(1)

## NAME

acton-doc --- Look up built-in reference documentation from the CLI

## SYNOPSIS

`acton doc` [_options_] _command_

## DESCRIPTION

Look up built-in reference documentation from the terminal.

At the moment, `acton doc` provides TVM instruction lookup through the `tvm`
subcommand.

## SUBCOMMANDS

### acton doc tvm

Lookup TVM instructions by exact name or fuzzy search query.

#### Synopsis

`acton doc tvm` [_options_] _instruction_...

#### Options

{{#options}}

{{#option "_instruction_..." }}
Instruction names or search queries.
{{/option}}

{{#option "`-f`, `--find`" }}
Treat the input as a fuzzy search query instead of an exact instruction name.
{{/option}}

{{#option "`-d`, `--description`" }}
Include descriptions in fuzzy matching.

Requires `--find`.
{{/option}}

{{#option "`--json`" }}
Emit JSON instead of formatted text.
{{/option}}

{{/options}}

#### Behavior

Without `--find`, each argument is treated as an instruction name.

- name matching is case-insensitive
- `-` and `#` are normalized for convenience
- JSON mode always returns an array of instruction objects
- at least one instruction name is required

With `--find`, each argument is treated as a search query.

- fuzzy matching uses instruction names by default
- `--description` also matches instruction descriptions, tags, and operands
- JSON mode returns query/matches objects
- at least one query is required
- queries with no matches return an error instead of an empty result set

## TOPICS

Currently available documentation namespaces:

- `tvm`

Additional namespaces may be added in future releases; `acton doc tvm` is the
current built-in reference surface.

## EXIT STATUS

- `0`: The requested documentation was printed successfully.
- `1`: The namespace was unknown, arguments were invalid, or rendering failed.

## DISPLAY OPTIONS

{{> options-display }}

## PROJECT OPTIONS

{{> options-project-pass-through }}

## EXAMPLES

1. Exact lookup:

   ```bash
   acton doc tvm ADD
   acton doc tvm ADD SUB
   ```

2. JSON output:

   ```bash
   acton doc tvm SENDRAWMSG --json
   ```

3. Fuzzy search:

   ```bash
   acton doc tvm SENRAWMSG --find
   acton doc tvm outcomng --find --description
   ```

4. Compare exact lookup with fuzzy search:

   ```bash
   acton doc tvm SENDRAWMSG
   acton doc tvm SENRAWMSG --find
   ```

## SEE ALSO

- [CLI docs lookup guide](https://ton-blockchain.github.io/acton/docs/commands/doc)
