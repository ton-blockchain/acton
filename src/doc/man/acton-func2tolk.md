# acton-func2tolk(1)

## NAME

acton-func2tolk --- Convert FunC sources to Tolk through the upstream converter

## SYNOPSIS

`acton func2tolk` [_options_] _path_

## DESCRIPTION

Convert a FunC source file or a directory of FunC files into Tolk.

Acton shells out to `npx` and runs the upstream
`@ton/convert-func-to-tolk` package. Standard input, output, and error are
forwarded directly to the converter process.

## OPTIONS

### Conversion Options

{{#options}}

{{#option "_path_" }}
Path to a `.fc` or `.func` file, or to a directory containing such files.
{{/option}}

{{#option "`--output` _path_" }}
Write converted output to the specified path.
{{/option}}

{{#option "`--warnings-as-comments`" }}
Insert warning comments into the generated output instead of reporting warnings
only on stderr.
{{/option}}

{{#option "`--no-camel-case`" }}
Disable snake_case to camelCase renaming.
{{/option}}

{{#option "`--version` _version_" }}
Version of `@ton/convert-func-to-tolk` to execute.
{{/option}}

{{/options}}

### Display Options

{{> options-display }}

### Project Options

{{> options-project-pass-through }}

## REQUIREMENTS

This command requires:

- Node.js
- npm
- `npx` available in `PATH`

Acton uses a temporary npm cache directory for each invocation.

## PROCESS MODEL

Acton runs:

```text
npx --yes @ton/convert-func-to-tolk@<version> ...
```

If the converter exits non-zero, `acton func2tolk` fails with the same outcome
reported as an Acton error.

## EXAMPLES

1. Convert all FunC files in a directory:

   ```bash
   acton func2tolk contracts
   ```

2. Convert a single file and keep warnings inline:

   ```bash
   acton func2tolk jetton-minter.fc --warnings-as-comments
   ```

3. Disable camelCase renaming:

   ```bash
   acton func2tolk jetton-minter.fc --no-camel-case
   ```

4. Use a specific converter version:

   ```bash
   acton func2tolk jetton-minter.fc --version 1.0.0
   ```

## SEE ALSO

- [convert-func-to-tolk](https://github.com/ton-blockchain/convert-func-to-tolk)
