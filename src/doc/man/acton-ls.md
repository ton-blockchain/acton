# acton-ls(1)

## Name

acton-ls --- Run the Acton language server

## Synopsis

`acton ls` [_options_]

## Description

Start the language server used for TON language tooling.

The server boots with the resolved project root, loads import path mappings from
`Acton.toml` when available, initializes the bundled standard library from
`.acton/tolk-stdlib`, and serves requests either over stdio or a local TCP
socket.

## Options

### LSP Options

{{#options}}

{{#option "`--port` _port_" }}
Listen on `127.0.0.1:<port>` and accept a TCP client.
{{/option}}

{{#option "`--stdio`" }}
Use stdio transport.
{{/option}}

{{#option "`--log-file` _path_" }}
Write language-server logs to a custom file.
{{/option}}

{{#option "`--no-log`" }}
Disable language-server logging setup.
{{/option}}

{{/options}}

### Display Options

{{> options-display }}

### Project Options

{{> options-project-resolved }}

## Transport

- if `--port` is provided, Acton binds to `127.0.0.1:<port>` and serves the
  first accepted TCP connection
- if `--port` is omitted and `--stdio` is not passed, Acton defaults to stdio
- if `--stdio` is used, the server reads from stdin and writes to stdout
- the TCP mode is single-client; after that client disconnects, the server
  exits

## Logging

Unless `--no-log` is passed, the server configures file logging.

The default log path is:

```text
build/logs/tolk-language-server.log
```

Parent directories for a custom `--log-file` are created automatically.

## Project Context

Before starting the server, Acton:

- resolves the project root and manifest path
- loads `[import-mappings]` from `Acton.toml` when present
- preloads `.acton/tolk-stdlib/common.tolk`

## Exit Status

- `0`: The language server started successfully and served its session.
- `1`: Startup failed because transport binding, stdlib loading, manifest
  resolution, or log-file setup failed.

## Examples

1. Run the language server over stdio:

   ```bash
   acton ls --stdio
   ```

2. Listen on a TCP port:

   ```bash
   acton ls --port 9273
   ```

3. Use a custom log file:

   ```bash
   acton ls --port 9273 --log-file logs/tolk-ls.log
   ```

## See Also

- `acton help doctor`
- [Acton documentation](https://ton-blockchain.github.io/acton/docs/welcome)
