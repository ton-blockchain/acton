# acton-build(1)

## Name

acton-build --- Build all configured contracts or a selected contract

## Synopsis

`acton build` [_options_] [_contract-name_]

## Description

Compile contracts declared in `Acton.toml`, resolve their dependencies, and
write build artifacts for the requested build set.

By default, `acton build` compiles every configured contract. If you pass a
`_contract-name_`, Acton builds only that contract and its transitive
dependencies.

For each successful build, Acton writes a JSON artifact to the build output
directory and, when the contract config has an `output` path, also writes the
compiled `.boc` file there. Dependency helper files are emitted into the
generated-code directory, and optional Fift output can be written separately.

Contracts with `.boc` sources are treated as precompiled inputs: Acton loads
their code, includes them in dependency resolution, and skips recompilation.

If the project has no `[contracts]` section or the section is empty, the
command prints guidance and exits without compiling anything.

## Options

### Build Options

{{> options-build }}

### Display Options

{{> options-display }}

### Project Options

{{> options-project-build }}

## Configuration

`acton build` reads contracts from `Acton.toml`:

```toml
[contracts.Wallet]
display-name = "Wallet Contract"
src = "contracts/Wallet.tolk"
output = "Wallet.boc"
depends = ["Child"]
```

Optional default output paths can be configured in `[build]`:

```toml
[build]
out-dir = "build"
gen-dir = "gen"
output-fift = "build/fift"
```

CLI flags override config values for the current invocation.
For dependency helpers, a per-dependency `depends[].path` still overrides the
resolved `gen-dir` for that helper file.

## Outputs

Depending on command flags and project configuration, `acton build` may write:

- `<out-dir>/<contract-name>.json` with `code_boc64` and `hash`
- the configured contract `output` `.boc` file
- `<gen-dir>/<dependency>.code.tolk` helper files for dependencies by default
  (or a dependency-specific custom path when `depends[].path` is configured)
- `<output-fift>/<contract-name>.fif` for compiled `.tolk` contracts
- a DOT dependency graph file when `--graph` is passed

Existing output files at those paths are replaced with freshly generated
artifacts.

## Best-Effort Behavior

Dependency-graph failures such as missing contracts or circular dependencies
stop the command before the main compile loop starts, so those failures do not
produce partial build outputs.

After dependency resolution succeeds, `acton build` becomes best-effort:

- compile failures for one contract are recorded while other eligible contracts
  continue building
- artifact-writing failures are also collected instead of aborting immediately
- artifacts that were written successfully before a later failure remain on disk

If an earlier dependency failed, its generated helper file is not produced. A
later parent contract may then fail because its generated import is missing.

When you build a specific `_contract-name_`, Acton limits the build set to that
contract and its transitive dependencies.

## Side Effects

`acton build` writes artifacts, cache entries, and optional graph output under
the resolved project root. If one contract fails after earlier contracts were
built successfully, the successful artifacts remain on disk.

## Exit Status

- `0`: The command completed without compilation failures.
- `1`: The command failed because compilation, artifact writing, or dependency
  resolution failed for at least one requested contract.

## Examples

1. Build every configured contract:

   ```bash
   acton build
   ```

2. Build one contract and its dependencies:

   ```bash
   acton build Wallet
   ```

3. Rebuild with a cleared cache:

   ```bash
   acton build --clear-cache
   ```

4. Write a dependency graph:

   ```bash
   acton build --graph deps.dot
   ```

5. Override output locations for a single run:

   ```bash
   acton build --out-dir artifacts --gen-dir artifacts/gen --output-fift artifacts/fift
   ```

6. Print compiled code and hashes after the build:

   ```bash
   acton build --info
   ```

## See Also

- `acton help wrapper`
- [Build system configuration reference](https://ton-blockchain.github.io/acton/docs/build-system/configuration-reference)
- [Acton.toml reference](https://ton-blockchain.github.io/acton/docs/acton-toml)
