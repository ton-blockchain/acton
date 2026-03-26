# acton-run(1)

## NAME

acton-run --- Run a named script from `Acton.toml`

## SYNOPSIS

`acton run` [_options_] _script_ [_args_...]

## DESCRIPTION

Run a command from the `[scripts]` section of `Acton.toml`.

This is similar to `npm run`: it lets you define short aliases for common
commands, multi-step workflows, or shell snippets and then execute them through
Acton.

## OPTIONS

### Run Options

{{#options}}

{{#option "_script_" }}
Name of the script entry in `[scripts]`.
{{/option}}

{{#option "_args_..." }}
Arguments appended to the configured script command.
{{/option}}

{{/options}}

### Display Options

{{> options-display }}

### Project Options

{{> options-project-resolved }}

## CONFIGURATION

Scripts are defined in `Acton.toml`:

```toml
[scripts]
deploy = "acton script scripts/deploy.tolk --broadcast"
test-unit = "acton test tests/unit"
custom-task = "echo 'Running custom task...'"
```

## BEHAVIOR

- extra arguments passed after the script name are appended to the configured
  command
- if the script exits non-zero, `acton run` exits with the same code
- on Unix-like systems scripts are executed via `sh -c`
- on Windows scripts are executed via `cmd /C`

## EXAMPLES

1. Run a configured script:

   ```bash
   acton run deploy
   ```

2. Append extra arguments:

   ```bash
   acton run deploy -- --net mainnet
   ```

3. Run a custom shell task:

   ```bash
   acton run custom-task
   ```

## SEE ALSO

- `acton help script`
- [Run command guide](https://ton-blockchain.github.io/acton/docs/commands/run)
