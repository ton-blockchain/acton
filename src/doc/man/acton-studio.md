# acton-studio(1)

## Name

acton-studio --- Open the browser workspace for an Acton project

## Synopsis

`acton studio` [_options_]

## Description

Available since Acton 1.2.

Start Acton Studio for the selected project and open it in the default browser.
Studio provides test history, contract inspection, wallets, and local environments.
It listens on `127.0.0.1:3015` by default. Keep the command running while using Studio.

Workspace metadata, test history, and managed environment data are stored under
`.studio/` in the project. Docker is required for Localnet environments;
Simulator environments do not require Docker.

## Options

{{#options}}

{{#option "`--host` _ip_" }}
Address for the Studio server. Only loopback addresses are accepted.
{{/option}}

{{#option "`--port` _port_" }}
Port for the Studio server. Use a different port if the default is occupied.
{{/option}}

{{#option "`--no-open`" }}
Start Studio without opening the default browser.
{{/option}}

{{/options}}

### Project Options

{{> options-project-resolved }}

## Test Reporting

CLI test runs report to Studio when it is running for the same project.
Reporting failures do not change test success or the exit status.
Use `acton test --no-studio-reporting` or `[test].studio-reporting = false`
to disable reporting.

## Examples

```bash
acton studio
acton studio --port 3020 --no-open
acton --project-root ../my-project studio
```

## See Also

- [Studio guide](https://ton-blockchain.github.io/acton/docs/studio/getting-started)
- `acton help test`
- `acton help simulator`
- `acton help localnet`
