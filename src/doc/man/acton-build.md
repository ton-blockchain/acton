# acton-build(1)

## NAME

acton-build --- Build all configured contracts or a selected contract

## SYNOPSIS

`acton build` [_options_] [_contract-id_]

## DESCRIPTION

Compile contracts declared in `Acton.toml`, resolve their dependencies, and
write build artifacts for the requested build set.

By default, `acton build` compiles every configured contract. If you pass a
`_contract-id_`, Acton builds only that contract and its transitive
dependencies.

For each successful build, Acton writes a JSON artifact to the build output
directory and, when the contract config has an `output` path, also writes the
compiled `.boc` file there. Dependency helper files are emitted into the
generated-code directory, and optional Fift output can be written separately.

Contracts with `.boc` sources are treated as precompiled inputs: Acton loads
their code, includes them in dependency resolution, and skips recompilation.

If the project has no `[contracts]` section or the section is empty, the
command prints guidance and exits without compiling anything.

## OPTIONS

### Build Options

{{> options-build }}

### Display Options

{{> options-display }}

### Project Options

{{> options-project-build }}

## CONFIGURATION

`acton build` reads contracts from `Acton.toml`:

```toml
[contracts.wallet]
name = "Wallet Contract"
src = "contracts/wallet.tolk"
output = "wallet.boc"
depends = ["child"]
```

Optional default output paths can be configured in `[build]`:

```toml
[build]
out-dir = "build"
gen-dir = "gen"
output-fift = "build/fift"
```

CLI flags override config values for the current invocation.

## OUTPUTS

Depending on command flags and project configuration, `acton build` may write:

- `<out-dir>/<contract-id>.json` with `code_boc64` and `hash`
- the configured contract `output` `.boc` file
- `<gen-dir>/<dependency>_code.tolk` helper files for dependencies
- `<output-fift>/<contract-id>.fif` for compiled `.tolk` contracts
- a DOT dependency graph file when `--graph` is passed

Existing output files at those paths are replaced with freshly generated
artifacts.

## SIDE EFFECTS

`acton build` writes artifacts, cache entries, and optional graph output under
the resolved project root. If one contract fails after earlier contracts were
built successfully, the successful artifacts remain on disk.

## EXIT STATUS

- `0`: The command completed without compilation failures.
- `1`: The command failed because compilation, artifact writing, or dependency
  resolution failed for at least one requested contract.

## EXAMPLES

1. Build every configured contract:

   ```bash
   acton build
   ```

2. Build one contract and its dependencies:

   ```bash
   acton build wallet
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

## SEE ALSO

- `acton help wrapper`
- [Build system configuration reference](https://ton-blockchain.github.io/acton/docs/build-system/configuration-reference)
- [Acton.toml reference](https://ton-blockchain.github.io/acton/docs/acton-toml)
