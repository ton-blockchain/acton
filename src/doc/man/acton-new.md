# acton-new(1)

## Name

acton-new --- Create a new Acton project

## Synopsis

`acton new` [_options_] _path_

## Description

Create a new Acton project in the given directory.

The command creates the target directory if it does not exist, copies the
selected template scaffold, writes starter project files such as `Acton.toml`,
`.env`, `.editorconfig`, and `.gitignore`, installs the bundled standard
library, and optionally initializes Git hooks and `AGENTS.md` guidance.

If `_path_` already exists as a directory, `acton new` fails instead of trying
to merge into that directory, even when it is empty. `acton new .` is the
explicit exception: it scaffolds into the current directory and may overwrite
existing files whose paths collide with the selected template.

If `git` is available in `PATH`, `acton new` runs `git init` in the project
directory and then runs `git add .`, which stages all current-directory
changes, not only files created by the scaffold.

The command does not create an initial commit.

## Options

### New Options

{{> options-new }}

### Display Options

{{> options-display }}

### Project Options

{{> options-project }}

## Templates

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

With `--app`, Acton also creates a Vite-based React app, a generated
TypeScript wrapper, and top-level npm metadata files.

### jetton

Jetton minter and wallet template with:

- multi-contract scaffold
- wrappers and tests
- deployment script
- CI workflow
- optional `AGENTS.md`

### nft

NFT collection and item template with:

- collection and item contracts
- wrappers and tests
- deployment scripts
- CI workflow
- optional `AGENTS.md`

## Interactive Mode

When enough information is missing and standard input/output are connected to a
terminal, `acton new` uses a short default flow:

- project name
- template
- whether to include the TypeScript app layout when the template supports it
- whether to configure advanced options

If you opt into advanced options, Acton can then prompt for:

- description
- license
- whether to install the default Git hooks when `git` is available
- whether to include `AGENTS.md`

If you skip advanced options, Acton keeps the default description `A TON
blockchain project`, the default license `MIT`, and leaves optional features
disabled unless their flags are passed explicitly.

In non-interactive mode, optional features stay disabled unless their flags are
passed explicitly. For CI or scripts, pass `--name`, `--description`,
`--template`, and `--license` if you want to avoid prompts entirely.

## Files

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
- wrappers that usually include helper shapes such as `fromStorage(...)`,
  `deploy(...)`, `send{Name}(...)`, `sendAny(...)`, and typed get-method calls
- deployment scripts
- `[scripts]` aliases such as `deploy-emulation` and `deploy-testnet` in `Acton.toml`
- frontend files for `--app`
- `.githooks/pre-commit` for `--hooks`
- `AGENTS.md` for `--agents`

## Counter App Layout

When `acton new --template counter --app` is used, the project includes:

- `contracts/src` for contract sources and shared Tolk types
- `contracts/tests` for tests and generated Tolk wrappers
- `contracts/scripts` for deployment and utility scripts
- `wrappers-ts/` for the generated TypeScript wrapper used by the app
- `app/` for the React + Vite frontend
- top-level `package.json` and `package-lock.json` for the frontend toolchain

Before running frontend commands, install the app dependencies:

```bash
npm ci
```

The generated app scaffold is a real frontend workspace, not just static demo
files. After `npm ci`, use the usual frontend lifecycle commands from the
generated `package.json` alongside normal Acton contract commands.

Typical commands in that generated app workspace:

- `npm run dev` to start the Vite development server
- `npm run build` to build both contracts and the frontend bundle
- `npm run preview` to preview the production bundle locally
- `npm run typecheck` for TypeScript checking
- `npm test` to run the bundled `acton test` command

## Side Effects

`acton new` writes only inside the chosen target directory. When `_path_` is
`.`, that means the current directory itself, and existing files with the same
paths as template files can be overwritten.

It also installs `.acton/tolk-stdlib` there, may create `.githooks/` plus an
`AGENTS.md` file when requested, and, when `git` is available, initializes the
project repository and runs `git add .`, which stages all current-directory
contents.

The command does not create a commit and does not modify parent directories.

## Exit Status

- `0`: The project scaffold was created successfully.
- `1`: Project creation failed because the target path already existed, a
  prompt could not be completed, or a filesystem/setup step failed.

## Examples

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

## See Also

- `acton help init`
- [Project initialization guide](https://ton-blockchain.github.io/acton/docs/tutorial/project-init)
- [Build system configuration reference](https://ton-blockchain.github.io/acton/docs/building/reference)
