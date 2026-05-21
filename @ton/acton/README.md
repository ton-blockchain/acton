# @ton/acton

TypeScript companion library for Acton projects.

The first version is intentionally small: it connects generated Acton TypeScript wrappers to
`acton localnet` without bundling a TON emulator into the package.
When `projectRoot` is omitted, `Localnet.start()` walks up from the current working directory and
uses the first directory that contains `Acton.toml` or `.git`.

## Counter-style test

Generate a wrapper first:

```bash
acton wrapper Counter --ts
```

Then use an explicit provider in a test:

```ts
import {expect, test} from "bun:test"
import {Localnet, ton} from "@ton/acton"

import {Counter} from "../wrappers-ts/Counter.gen"

const localnet = await Localnet.start()

test("counter increases", async () => {
  const deployer = localnet.treasury("deployer")
  const counter = localnet.contract(
    Counter.fromStorage({
      id: 0n,
      owner: deployer.address,
      counter: 0n,
    }),
  )

  await counter.sendDeploy(deployer, ton("0.05"))
  await counter.sendIncreaseCounter(deployer, ton("0.02"), {increaseBy: 1n})

  expect(await counter.getCurrentCounter()).toBe(1n)
})
```

`treasury` is a synthetic local sender. It is useful for local tests because the source
account is not signed or debited. Real wallet/deploy helpers will live on top of the same localnet
client API.

`contract` returns a provider-bound handle for Acton generated wrappers. It binds only wrapper
methods whose names start with `send` or `get`; `provider` remains available for lower-level calls.

`Localnet.start()` auto-closes the spawned `acton localnet` process when the test process exits.
Call `localnet.close()` only when you need to stop it earlier.

In `bun test`, `Localnet.start()` also snapshots the initial localnet state and restores it after
each test. Pass `{autoReset: false}` if a suite intentionally shares chain state between tests.
