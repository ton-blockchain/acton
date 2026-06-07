# acton-add(1)

## Name

acton-add --- Add resources to the current Acton project

## Synopsis

`acton add` [_options_] _command_

## Description

Add reusable resources to an existing Acton project.

The first supported flow is adding contracts from built-in templates. This
copies the selected template's contract sources, wrappers, tests, and scripts
into the current project and registers the template contracts in `Acton.toml`.

Template files are copied into a template-specific namespace such as
`contracts/counter/` or `contracts/jetton/`. Imports inside copied Tolk files
are rewritten to point at that namespace, so template support files such as
`types.tolk`, `errors.tolk`, and wrapper utilities do not collide with files
already present in the project.

The command refuses to overwrite existing files unless `--overwrite` is passed.
It also refuses to add a template when any of its contract ids already exist in
`Acton.toml`.

## Subcommands

### acton add contract

Add a contract and its supporting template files to the current project.

#### Synopsis

`acton add contract` [_options_] `--from template` [_template_]

#### Options

{{#options command="acton add contract"}}

{{#option "`--from` _source_" }}
Source to add the contract from.

Currently only `template` is supported.
{{/option}}

{{#option "_template_" }}
Template to add.

If omitted in an interactive terminal, Acton asks which template to use.
In non-interactive mode, this argument is required.
{{/option}}

{{#option "`--overwrite`" }}
Overwrite existing files whose paths collide with the template.

This only applies to files on disk. Existing contract ids in `Acton.toml` are
not overwritten.
{{/option}}

{{/options}}

## Display Options

{{> options-display }}

## Project Options

{{> options-project-resolved }}

## Templates

Supported template values are the same built-in contract templates used by
`acton new`:

- `empty`
- `counter`
- `jetton`
- `nft`
- `w5-extension`

## Examples

1. Add the counter template to the current project:

   ```bash
   acton add contract --from template counter
   ```

2. Add the jetton template and build its minter contract:

   ```bash
   acton add contract --from template jetton
   acton build JettonMinter
   ```

3. Run tests copied from a template:

   ```bash
   acton test tests/counter/counter.test.tolk
   ```

