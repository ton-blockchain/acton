import {
  expectFailedTx,
  expectSuccessfulDeploy,
  expectSuccessfulTx,
  test,
  ton,
  type ActonTestContext,
} from "../../../src/test.ts"
import {Counter} from "../wrappers-ts/Counter.gen.ts"

test("prints transaction tree when from does not match", async ({localnet}) => {
  const {contract, deployer, notDeployer} = await setupTest({localnet})

  const res = await contract.sendIncreaseCounter(
    deployer,
    ton("0.1"),
    {increaseBy: 123n},
    {bounce: false},
  )

  expectSuccessfulTx(res, {
    from: notDeployer.address,
    to: contract.address,
  })
})

test("prints exit code mismatch for successful transaction", async ({localnet}) => {
  const {contract, deployer} = await setupTest({localnet})

  const res = await contract.sendIncreaseCounter(
    deployer,
    ton("0.1"),
    {increaseBy: 1n},
    {bounce: false},
  )

  expectFailedTx(res, {
    from: deployer.address,
    to: contract.address,
    exitCode: Counter.Errors["Errors.NotOwner"],
  })
})

async function setupTest({localnet}: ActonTestContext) {
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
