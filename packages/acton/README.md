# @ton/acton

TypeScript companion library for Acton projects.

The first version is intentionally small: it connects generated Acton TypeScript wrappers to
`acton localnet` without bundling a TON emulator into the package.
When `projectRoot` is omitted, `Localnet.start()` walks up from the current working directory and
uses the first directory that contains `Acton.toml` or `.git`.

## Counter-style localnet script

Generate a wrapper first:

```bash
acton wrapper Counter --ts
```

Then use the generated wrapper against `acton localnet`:

```ts
import {Localnet, ton} from "@ton/acton"

import {Counter} from "../wrappers-ts/Counter.gen"

const localnet = await Localnet.start()
try {
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

  console.log(await counter.getCurrentCounter())
} finally {
  await localnet.close()
}
```

`treasury` is a synthetic local sender for local development. The source account is not signed or
debited. Real wallet and deploy helpers can be built on top of the same localnet client API.

`contract` returns a provider-bound handle for Acton generated wrappers. It binds only wrapper
methods whose names start with `send` or `get`; `provider` remains available for lower-level calls.

`Localnet.start()` registers process-exit cleanup for the spawned `acton localnet` process. Call
`localnet.close()` explicitly when the script has finished so resources are released
deterministically.
