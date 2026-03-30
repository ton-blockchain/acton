# acton-compile(1)

## NAME

acton-compile --- Compile a Tolk file into TVM code and related artifacts

## SYNOPSIS

`acton compile` [_options_] _path_

## DESCRIPTION

Compile a single `.tolk` source file with the Tolk compiler and print the
resulting code information or write artifacts to files.

This command is useful for inspecting compiler output, producing standalone BoC
files, generating source maps for debugging, and exporting ABI or Fift output
without running the full project build pipeline.

## OPTIONS

### Compile Options

{{#options}}

{{#option "_path_" }}
Path to the Tolk source file to compile.
{{/option}}

{{#option "`--json`" }}
Print the compilation result as structured JSON.
{{/option}}

{{#option "`--base64-only`" }}
Print only the compiled code as base64.
{{/option}}

{{#option "`--boc` _path_" }}
Write the compiled code to a binary BoC file.
{{/option}}

{{#option "`--fift` _path_" }}
Write the generated Fift representation to a file.
{{/option}}

{{#option "`--source-map` _path_" }}
Write a source map file and enable debug-oriented compilation output.
{{/option}}

{{#option "`--abi` _path_" }}
Write the emitted contract ABI to a file.
{{/option}}

{{#option "`--clear-cache`" }}
Clear the compilation cache before compiling.
{{/option}}

{{/options}}

### Display Options

{{> options-display }}

### Project Options

{{> options-project-resolved }}

## OUTPUT

Without file-output flags, `acton compile` prints compilation information to
standard output.

`--json` and `--base64-only` change the stdout format when stdout output is
still enabled. File-output flags such as `--fift`, `--source-map`, and `--abi`
can be combined with either stdout mode.

`--boc` is special: Acton writes the binary BoC file and returns successfully
without printing the usual stdout payload.

Depending on the selected options, Acton can also write:

- a binary BoC file via `--boc`
- a Fift file via `--fift`
- a source map via `--source-map`
- an ABI file via `--abi`

`--source-map` enables debug-oriented compilation output. If the compiler does
not emit a contract ABI for the input file, `--abi` does not create an ABI
artifact.

## CACHE

Acton uses a compilation cache to speed up repeated runs.

- Use `--clear-cache` to force recompilation.
- Cache entries are invalidated automatically when source inputs change.

## EXIT STATUS

- `0`: Compilation completed successfully, including runs that emitted warnings.
- `1`: Compilation failed, cache setup failed, input validation failed, or an
  output artifact could not be written.

## EXAMPLES

1. Compile a Tolk file and print the result:

   ```bash
   acton compile contracts/main.tolk
   ```

2. Save the compiled code to a BoC file:

   ```bash
   acton compile contracts/main.tolk --boc main.boc
   ```

3. Emit source map and ABI artifacts:

   ```bash
   acton compile contracts/main.tolk --source-map main.map.json --abi main.abi.json
   ```

4. Print machine-readable JSON:

   ```bash
   acton compile contracts/main.tolk --json
   ```

5. Generate artifacts for source-correlated disassembly:

   ```bash
   acton compile contracts/main.tolk --boc build/main.boc --source-map build/main.map.json
   ```

## SEE ALSO

- `acton help build`
- `acton help disasm`
- [Build system configuration reference](https://ton-blockchain.github.io/acton/docs/build-system/configuration-reference)
