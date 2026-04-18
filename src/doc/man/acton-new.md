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
to merge into that directory, even when it is empty. `acton new .` is the
explicit exception: it scaffolds into the current directory and may overwrite
existing files whose paths collide with the selected template.

If `git` is available in `PATH`, `acton new` runs `git init` in the project
directory and then runs `git add .`, which stages all current-directory
changes, not only files created by the scaffold.

The command does not create an initial commit.

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

## INTERACTIVE MODE

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
- wrappers that usually include helper shapes such as `fromStorage(...)`,
  `deploy(...)`, `send{Name}(...)`, `sendAny(...)`, and typed get-method calls
- deployment scripts
- `[scripts]` aliases such as `deploy-emulation` and `deploy-testnet` in `Acton.toml`
- frontend files for `--app`
- `.githooks/pre-commit` for `--hooks`
- `AGENTS.md` for `--agents`

Every scaffold also writes `Acton.toml` script aliases for the template's main
deploy script:

- `deploy-emulation` runs the generated deploy script locally via `acton script`
- `deploy-testnet` runs the same deploy script with `--net testnet`

That gives new projects an immediate `acton run ...` entrypoint without having
to hand-author a `[scripts]` section first.

Wrapper files copied during scaffolding are template-owned starter files. They
are not regenerated during `acton new`, so the exact helper surface can differ
slightly from what the current `acton wrapper` generator would emit for the
same contract today.

Each built-in template also ships its own optional `AGENTS.md` guidance. When
you pass `--agents`, Acton copies the selected template's version into the
project root. This is a one-time scaffolded file, not a live link back to the
template, so later template updates do not automatically rewrite your local
`AGENTS.md`.

## COUNTER APP LAYOUT

When `acton new --template counter --app` is used, the project includes:

- `contracts/src` for contract sources and shared Tolk types
- `contracts/tests` for tests and generated Tolk wrappers
- `contracts/scripts` for deployment and utility scripts
- `wrappers/` for the generated TypeScript wrapper used by the app
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

## SIDE EFFECTS

`acton new` writes only inside the chosen target directory. When `_path_` is
`.`, that means the current directory itself, and existing files with the same
paths as template files can be overwritten.

It also installs `.acton/tolk-stdlib` there, may create `.githooks/` plus an
`AGENTS.md` file when requested, and, when `git` is available, initializes the
project repository and runs `git add .`, which stages all current-directory
contents.

The command does not create a commit and does not modify parent directories.

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
