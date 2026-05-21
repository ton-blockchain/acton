import {expect, test} from "bun:test"
import {beginCell, Cell} from "@ton/core"

import {
  expectFailedTx,
  expectSuccessfulDeploy,
  expectSuccessfulTx,
  Localnet,
  ton,
  type ContractHandle,
} from "../../../src/index.ts"
import {Counter} from "../wrappers-ts/Counter.gen.ts"

const localnet = await Localnet.start()
const defaultSendValue = ton("0.1")

type CounterContract = ContractHandle<Counter>
type Treasury = ReturnType<Localnet["treasury"]>

type TestContext = {
  readonly contract: CounterContract
  readonly deployer: Treasury
  readonly notDeployer: Treasury
}

test("deploy exposes initial owner", async () => {
  const {contract, deployer} = await setupTest()

  const owner = await contract.getOwner()
  expect(owner.equals(deployer.address)).toBe(true)
})

test("increase counter", async () => {
  const {contract, deployer} = await setupTest()

  const res = await contract.sendIncreaseCounter(
    deployer,
    defaultSendValue,
    {increaseBy: 123n},
    {bounce: false},
  )
  expectSuccessfulTx(res, {
    from: deployer.address,
    to: contract.address,
  })

  expect(await contract.getCurrentCounter()).toBe(123n)
})

test("reset counter", async () => {
  const {contract, deployer} = await setupTest()

  const res = await contract.sendIncreaseCounter(
    deployer,
    defaultSendValue,
    {increaseBy: 123n},
    {bounce: false},
  )
  expectSuccessfulTx(res, {
    from: deployer.address,
    to: contract.address,
  })

  expect(await contract.getCurrentCounter()).toBe(123n)

  const res2 = await contract.sendResetCounter(deployer, defaultSendValue, {}, {bounce: false})
  expectSuccessfulTx(res2, {
    from: deployer.address,
    to: contract.address,
  })

  expect(await contract.getCurrentCounter()).toBe(0n)
})

test("decrease counter", async () => {
  const {contract, deployer} = await setupTest()

  const res = await contract.sendIncreaseCounter(
    deployer,
    defaultSendValue,
    {increaseBy: 123n},
    {bounce: false},
  )
  expectSuccessfulTx(res, {
    from: deployer.address,
    to: contract.address,
  })

  const res2 = await contract.sendDecreaseCounter(
    deployer,
    defaultSendValue,
    {decreaseBy: 23n},
    {bounce: false},
  )
  expectSuccessfulTx(res2, {
    from: deployer.address,
    to: contract.address,
  })

  expect(await contract.getCurrentCounter()).toBe(100n)
})

test("decrease counter fails on underflow", async () => {
  const {contract, deployer} = await setupTest()

  const res = await contract.sendDecreaseCounter(
    deployer,
    defaultSendValue,
    {decreaseBy: 1n},
    {bounce: false},
  )
  expectFailedTx(res, {
    from: deployer.address,
    to: contract.address,
    exitCode: Counter.Errors["Errors.CounterUnderflow"],
  })
})

test("decrease counter fails on overflow", async () => {
  const {contract, deployer} = await setupTest()

  const res = await contract.sendIncreaseCounter(
    deployer,
    defaultSendValue,
    {increaseBy: 1n},
    {bounce: false},
  )
  expectSuccessfulTx(res, {
    from: deployer.address,
    to: contract.address,
  })

  const res2 = await contract.sendDecreaseCounter(
    deployer,
    defaultSendValue,
    {decreaseBy: 0xff_ff_ff_ffn},
    {bounce: false},
  )
  expectFailedTx(res2, {
    from: deployer.address,
    to: contract.address,
    exitCode: Counter.Errors["Errors.CounterUnderflow"],
  })

  expect(await contract.getCurrentCounter()).toBe(1n)
})

test("non-owner cannot change counter", async () => {
  const {contract, notDeployer} = await setupTest()

  const res = await contract.sendIncreaseCounter(
    notDeployer,
    defaultSendValue,
    {increaseBy: 123n},
    {bounce: false},
  )
  expectFailedTx(res, {
    from: notDeployer.address,
    to: contract.address,
    exitCode: Counter.Errors["Errors.NotOwner"],
  })

  expect(await contract.getCurrentCounter()).toBe(0n)
})

test("unknown message", async () => {
  const {contract, deployer} = await setupTest()

  const res = await sendAny(contract, deployer, beginCell().storeInt(0x00_00_09_99, 32).endCell())
  expectFailedTx(res, {
    from: deployer.address,
    to: contract.address,
    exitCode: Counter.Errors["Errors.InvalidMessage"],
  })

  const res2 = await sendAny(contract, deployer, Cell.EMPTY)
  expectSuccessfulTx(res2, {
    from: deployer.address,
    to: contract.address,
  })
})

async function setupTest(): Promise<TestContext> {
  const deployer = localnet.treasury("deployer")
  const notDeployer = localnet.treasury("notDeployer")
  const contract = localnet.contract(
    Counter.fromStorage({
      counter: 0n,
      id: 0n,
      owner: deployer.address,
    }),
  )

  const res = await contract.sendDeploy(deployer, ton("1"), {bounce: false})
  expectSuccessfulDeploy(res, {
    to: contract.address,
  })

  return {contract, deployer, notDeployer}
}

async function sendAny(contract: CounterContract, from: Treasury, body: Cell) {
  return localnet.trackTransactions(contract.address, async () => {
    await from.send({
      body,
      bounce: false,
      to: contract.address,
      value: defaultSendValue,
    })
  })
}
