# acton-wrapper(1)

## Name

acton-wrapper --- Generate Tolk or TypeScript wrappers for a contract

## Synopsis

`acton wrapper` [_options_] _contract-name_

`acton wrapper` [_options_] `--all`

## Description

Generate a wrapper for the contract identified by `_contract-name_` from
`Acton.toml`.

Use `--all` to generate wrappers for every contract defined in `Acton.toml`
without picking a single one.

Wrapper generation uses the ABI emitted by the Tolk compiler. In practice, the
contract header is the source of truth for typed storage accessors, incoming
message helpers, and generated get-method bindings.

The command can also generate a stub test file or emit a TypeScript wrapper for
frontend and tooling integrations.

`acton wrapper` compiles the selected contract directly. A prior `acton build`
run is not required.

## Options

### Wrapper Options

{{#options}}

{{#option "_contract-name_" }}
Contract name from `Acton.toml` to generate the wrapper for. Required unless
`--all` is given.
{{/option}}

{{#option "`--all`" }}
Generate wrappers for every contract defined in `Acton.toml`.

Conflicts with _contract-name_, `--output`, and `--test-output` (which
designate a single output file).
{{/option}}

{{#option "`-o`, `--output` _path_" }}
Write the generated wrapper to an exact path.

Conflicts with `--output-dir` and `--all`.
{{/option}}

{{#option "`--output-dir` _dir_" }}
Write the generated wrapper to a directory and let Acton choose the file name.

Conflicts with `--output`.
{{/option}}

{{/options}}

### Test Stub Options

{{#options}}

{{#option "`-t`, `--test`" }}
Generate a stub test file together with the wrapper.
{{/option}}

{{#option "`--test-output` _path_" }}
Write the generated test file to an exact path.

Requires `--test`. Conflicts with `--all`.
{{/option}}

{{#option "`--test-output-dir` _dir_" }}
Write the generated test file to a directory and let Acton choose the file
name.

Requires `--test` and conflicts with `--test-output`.
{{/option}}

{{/options}}

### TypeScript Options

{{#options}}

{{#option "`--ts`" }}
Generate a TypeScript wrapper through `@ton/tolk-abi-to-typescript@0.5.0`.

Conflicts with test stub generation.
{{/option}}

{{/options}}

### Display Options

{{> options-display }}

### Project Options

{{> options-project-resolved }}

## ABI Requirements

Wrapper generation depends on the contract ABI exposed by the Tolk compiler.

- `storage: ...` enables typed storage helpers such as `fromStorage`
- `incomingMessages: ...` enables `send{Message}` helpers
- declared get methods are emitted as wrapper methods

If `incomingMessages` is missing, message-sending helpers are not generated. If
`storage` is missing, storage helpers fall back to an untyped initializer.

## TypeScript Generation

`acton wrapper --ts` shells out to `npx @ton/tolk-abi-to-typescript@0.5.0`.

- Node.js, npm, and `npx` must be available in `PATH`
- existing wrapper files at the target path are overwritten
- `--ts` cannot be combined with `--test`, `--test-output`, or
  `--test-output-dir`

## Configuration

Project-wide defaults can be configured in `Acton.toml`:

```acton-toml title="Acton.toml"
[wrappers.tolk]
output-dir = "wrappers"
generate-test = true
test-output-dir = "tests"

[wrappers.typescript]
output-dir = "wrappers-ts"
```

CLI flags override config values for the current invocation.

## Exit Status

- `0`: Wrapper generation completed successfully.
- `1`: The contract could not be found or compiled, ABI data was missing, the
  TypeScript generator could not run, or an output file could not be written.

## Examples

1. Generate the default Tolk wrapper:

   ```bash
   acton wrapper Counter
   ```

2. Generate a wrapper and stub test:

   ```bash
   acton wrapper Counter --test
   ```

3. Generate a TypeScript wrapper:

   ```bash
   acton wrapper Counter --ts
   ```

4. Generate into custom locations:

   ```bash
   acton wrapper Counter --output-dir tests/generated
   acton wrapper Counter --test --test-output-dir tests/generated
   acton wrapper Counter --ts --output-dir wrappers-ts
   ```

5. Generate a frontend-oriented TypeScript wrapper layout:

   ```bash
   acton wrapper Counter --ts --output-dir app/src/wrappers-ts
   ```

6. Generate wrappers for every contract in `Acton.toml`:

   ```bash
   acton wrapper --all
   acton wrapper --all --ts
   acton wrapper --all --output-dir wrappers/generated
   ```

## See Also

- `acton help build`
- `acton help test`
- [Generating wrappers guide](https://ton-blockchain.github.io/acton/docs/testing/generating-wrappers)
