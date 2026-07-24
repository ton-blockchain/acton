# @ton/acton Architecture

`@ton/acton` keeps TypeScript tests thin. The library starts or connects to `acton localnet`,
uses generated Acton TypeScript wrappers for contract calls, and reports structured events back to
`acton test` so the existing Acton reporter, coverage, trace dump, and UI machinery stay in use.

```mermaid
flowchart TD
  user["User runs acton test"]
  cli["Acton CLI test command"]
  discover["Discover tests\n.tolk and .test.ts"]
  tolk["Native Tolk test runner"]
  rustRunner["Rust TestRunner"]
  worker["Bun worker\n@ton/acton/test-worker"]
  importTests["Import TS test file\nregister test() cases"]
  localnet["Localnet.start()\nspawn acton localnet"]
  wrappers["Generated Acton TS wrappers\nCounter.gen.ts"]
  handle["localnet.contract(wrapper)\nprovider-bound handle"]
  provider["LocalnetContractProvider"]
  http["Localnet HTTP API"]
  liteNode["acton localnet"]
  emulate["/api/emulate/v1/emulateTrace\ncoverage + trace diagnostics"]
  sendBoc["/api/v2/sendBocReturnHash\ncommit message"]
  getMethod["/api/v2/runGetMethod\ngetter execution"]
  events["Structured worker events\ncoverage, trace, treasury, result"]
  known["Known addresses\nfrom treasury()"]
  emulations["EmulationsState\ncoverage and trace records"]
  formatter["Acton formatter and reporters"]
  outputs["Console output\ncoverage report\ntrace JSON / UI"]

  user --> cli
  cli --> discover
  discover --> tolk
  discover --> rustRunner
  rustRunner --> worker
  worker --> importTests
  importTests --> localnet
  importTests --> wrappers
  wrappers --> handle
  handle --> provider
  provider --> http
  localnet --> liteNode
  http --> liteNode

  provider -->|"sendX"| emulate
  emulate --> sendBoc
  sendBoc --> liteNode
  provider -->|"getX"| getMethod
  getMethod --> liteNode

  worker --> events
  events --> rustRunner
  rustRunner --> known
  rustRunner --> emulations
  known --> formatter
  emulations --> formatter
  formatter --> outputs
  tolk --> formatter
```

## Responsibilities

`acton test` owns discovery, contract compilation, reporting, coverage aggregation, trace export,
and UI integration. TypeScript files are just another test source for the same runner.

`@ton/acton/test-worker` is the protocol boundary. It runs TS tests in Bun, snapshots and restores
localnet state between tests, and emits newline-delimited JSON events prefixed with
`__ACTON_NODE_EVENT__`.

`Localnet` is the client-side test facade. It starts `acton localnet`, provides synthetic
`treasury()` senders, binds generated wrapper methods through `contract()`, and collects diagnostics
only when `acton test` enables coverage or trace export.

The TS package does not bundle an emulator. All execution goes through `acton localnet`, which keeps
behavior aligned with the Acton CLI and lets TS tests reuse existing Acton output, coverage, and UI
features.
