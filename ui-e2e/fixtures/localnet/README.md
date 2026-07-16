# Visual localnet fixture

`ui-state.json` is the canonical state for localnet and explorer visual tests. Start it with:

```sh
acton localnet start --port 15411 --load-state ui-e2e/fixtures/localnet/ui-state.json --no-mining
```

The node runs with mining disabled so timestamps and block contents stay stable. Scenarios that
change state must use a worker-local copy/node or revert to the canonical snapshot before capture.

Parallel Playwright scenarios delay and then forward the real API request in the owning page, so
other workers are not affected. An isolated node can also be delayed at runtime with:

```sh
curl -X POST http://127.0.0.1:15411/acton_setNetworkConditions \
  -H 'content-type: application/json' \
  --data '{"response_delay_ms":1500}'
```

Reset a node-wide delay to `0` in test cleanup.
