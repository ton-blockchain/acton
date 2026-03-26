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

If `_path_` already exists as a directory, `acton new` fails instead of trying
to merge into that directory, even when it is empty. To scaffold into the
current directory explicitly, run `acton new .` for that case.

`acton new` does not run `git init`. It only writes files that work in a new or
already-versioned repository.

## OPTIONS

### New Options

{{> options-new }}

### Display Options

{{> options-display }}

### Project Options

{{> options-project }}

## TEMPLATES

### empty

Minimal project skeleton with:

- one starter contract
- generated wrapper and tests
- deployment script
- CI workflow
- optional `AGENTS.md`

### counter

Counter contract template with:

- one contract, wrapper, and tests
- deployment script
- CI workflow
- optional `AGENTS.md`

This template also supports the optional TypeScript app layout with `--app`.

### jetton

Jetton minter and wallet template with:

- multi-contract scaffold
- wrappers and tests
- deployment script
- CI workflow
- optional `AGENTS.md`

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
passed explicitly. For CI or scripts, pass `--name`, `--description`,
`--template`, and `--license` if you want to avoid prompts entirely.

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

## SIDE EFFECTS

`acton new` creates or overwrites files only inside the new project directory.
It also installs `.acton/tolk-stdlib` there and may create `.githooks/` plus
an `AGENTS.md` file when requested.

The command does not initialize a Git repository, does not commit files, and
does not modify parent directories.

## EXIT STATUS

- `0`: The project scaffold was created successfully.
- `1`: Project creation failed because the target path already existed, a
  prompt could not be completed, or a filesystem/setup step failed.

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

5. Create a project in the current directory:

   ```bash
   acton new . --template empty --name "My Project" --description "A TON blockchain project" --license MIT
   ```

## SEE ALSO

- `acton help init`
- [Project initialization guide](https://ton-blockchain.github.io/acton/docs/project-init)
- [Build system configuration reference](https://ton-blockchain.github.io/acton/docs/build-system/configuration-reference)
