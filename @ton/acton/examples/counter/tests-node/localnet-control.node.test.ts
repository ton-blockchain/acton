import {afterAll, afterEach, beforeAll, beforeEach, describe, expect, test} from "vitest"

import {
  expectSuccessfulDeploy,
  expectSuccessfulTx,
  Localnet,
  type LocalnetRecoveryPointResult,
  ton,
} from "../../../src/index.ts"
import {Counter} from "../wrappers-ts/Counter.gen.ts"

describe("localnet control", () => {
  let localnet: Localnet
  let snapshotId: LocalnetRecoveryPointResult | undefined

  beforeAll(async () => {
    localnet = await Localnet.start({
      autoReset: false,
    })
  })

  beforeEach(async () => {
    snapshotId = await localnet.snapshot()
  })

  afterEach(async () => {
    if (snapshotId) {
      await localnet.revert(snapshotId)
      snapshotId = undefined
    }
  })

  afterAll(async () => {
    await localnet.close()
  })

  test("updates localnet network conditions", async () => {
    const updated = await localnet.setNetworkConditions({responseDelayMs: 25})

    expect(updated.response_delay_ms).toBe(25)

    const nodeInfo = await localnet.nodeInfo()
    expect(nodeInfo.network_conditions.response_delay_ms).toBe(25)
  })

  test("deploys counter and applies internal messages", async () => {
    const deployer = localnet.treasury("vitest-deployer")
    const contract = localnet.contract(
      Counter.fromStorage({
        counter: 0n,
        id: 0n,
        owner: deployer.address,
      }),
    )

    const deployResult = await contract.sendDeploy(deployer, ton("1"), {bounce: false})
    expectSuccessfulDeploy(deployResult, {
      to: contract.address,
    })
    const owner = await contract.getOwner()
    expect(owner.equals(deployer.address)).toBe(true)
    expect(await contract.getCurrentCounter()).toBe(0n)

    const increaseResult = await contract.sendIncreaseCounter(
      deployer,
      ton("0.1"),
      {increaseBy: 7n},
      {bounce: false},
    )
    expectSuccessfulTx(increaseResult, {
      from: deployer.address,
      to: contract.address,
    })
    expect(await contract.getCurrentCounter()).toBe(7n)

    const decreaseResult = await contract.sendDecreaseCounter(
      deployer,
      ton("0.1"),
      {decreaseBy: 2n},
      {bounce: false},
    )
    expectSuccessfulTx(decreaseResult, {
      from: deployer.address,
      to: contract.address,
    })
    expect(await contract.getCurrentCounter()).toBe(5n)

    const resetResult = await contract.sendResetCounter(deployer, ton("0.1"), {}, {bounce: false})
    expectSuccessfulTx(resetResult, {
      from: deployer.address,
      to: contract.address,
    })
    expect(await contract.getCurrentCounter()).toBe(0n)
  })
})
