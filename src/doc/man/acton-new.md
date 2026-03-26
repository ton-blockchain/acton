# acton-new(1)

## NAME

acton-new --- Create a new Acton project

## SYNOPSIS

`acton new` [_options_] _path_

## DESCRIPTION

Create a new Acton project in the given directory.

The command creates the target directory if it does not exist, copies the
selected template scaffold, writes starter project files such as `Acton.toml`,
`.env`, `.editorconfig`, and `.gitignore`, installs the bundled standard
library, and optionally initializes Git hooks and `AGENTS.md` guidance.

If `_path_` already exists as a directory, `acton new` fails and explains how
to create a project in the current directory with `acton new .` instead.

## OPTIONS

### New Options

{{> options-new }}

### Display Options

{{> options-display }}

### Project Options

{{> options-project }}

## TEMPLATES

### empty

Minimal project skeleton with a starter contract, wrapper, tests, deployment
script, and CI workflow.

### counter

Counter contract template with tests, wrapper, deployment script, and CI
workflow.

This template also supports the optional TypeScript app layout with `--app`.

### jetton

Jetton minter and wallet template with wrappers, tests, deployment script, and
CI workflow.

## INTERACTIVE MODE

When enough information is missing and standard input/output are connected to a
terminal, `acton new` prompts for:

- project name
- description
- template
- license
- whether to include the TypeScript app layout when the template supports it
- whether to install the default Git hooks when `git` is available
- whether to include `AGENTS.md`

In non-interactive mode, optional features stay disabled unless their flags are
passed explicitly.

## FILES

The generated project always includes:

- `Acton.toml`
- `.acton/`
- `.env`
- `.editorconfig`
- `.gitignore`

Depending on the selected template and options, Acton may also generate:

- contract sources
- tests
- wrappers
- deployment scripts
- frontend files for `--app`
- `.githooks/pre-commit` for `--hooks`
- `AGENTS.md` for `--agents`

{{> section-exit-status }}

## EXAMPLES

1. Create a new project in `my-project`:

   ```bash
   acton new my-project
   ```

2. Create a non-interactive counter project with explicit metadata:

   ```bash
   acton new my-project --name "My Project" --description "Cool description" --template counter --license MIT
   ```

3. Create the counter template with the TypeScript app layout:

   ```bash
   acton new my-project --template counter --app
   ```

4. Create a project and include `AGENTS.md` guidance:

   ```bash
   acton new my-project --template empty --agents
   ```

## SEE ALSO

- `acton help init`
- [Project initialization guide](https://ton-blockchain.github.io/acton/docs/project-init)
- [Build system configuration reference](https://ton-blockchain.github.io/acton/docs/build-system/configuration-reference)
