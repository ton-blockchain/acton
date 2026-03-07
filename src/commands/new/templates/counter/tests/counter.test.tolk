import "@acton/emulation/network"
import "@acton/testing/expect"
import "@acton/testing/transaction_expect"

import "@wrappers/Counter"
import "@contracts/types"

get fun `test increase counter`() {
    val (contract, deployer, _) = setupTest();

    val res = contract.sendIncreaseCounter(deployer.address, 123);
    expect(res).toHaveSuccessfulTx<IncreaseCounter>({ from: deployer.address, to: contract.address });

    expect(contract.currentCounter()).toEqual(123)
}

get fun `test reset counter`() {
    val (contract, deployer, _) = setupTest();

    val res = contract.sendIncreaseCounter(deployer.address, 123);
    expect(res).toHaveSuccessfulTx<IncreaseCounter>({ from: deployer.address, to: contract.address });

    expect(contract.currentCounter()).toEqual(123);

    val res2 = contract.sendResetCounter(deployer.address);
    expect(res2).toHaveSuccessfulTx<ResetCounter>({ from: deployer.address, to: contract.address });

    expect(contract.currentCounter()).toEqual(0);
}

get fun `test unknown message`() {
    val (contract, deployer, _) = setupTest();

    // non-empty message
    val res = contract.sendAny(deployer.address, beginCell().storeInt(0x999, 32).endCell());
    expect(res).toHaveFailedTx({ from: deployer.address, to: contract.address, exitCode: 0xFFFF });

    // empty message
    val res2 = contract.sendAny(deployer.address, createEmptyCell());
    expect(res2).toHaveSuccessfulTx({ from: deployer.address, to: contract.address });
}

/// Initializes the test environment, creating a fresh instance of the contract.
/// Returns the contract wrapper and two treasury accounts (`deployer` and `not_deployer`).
fun setupTest(): (undefined, Treasury, Treasury) {
    // Create a treasury account for deployment (typically the owner)
    val deployer = net.treasury("deployer");
    // Create another treasury account for testing interactions from other users
    val notDeployer = net.treasury("not_deployer");

    // Initialize and deploy the contract with default values
    val contract = Counter.fromStorage({ id: 0, counter: 0 });
    val res = contract.deploy(deployer.address, { value: ton("1") });
    expect(res).toHaveSuccessfulDeploy({ to: contract.address });

    return (contract, deployer, notDeployer)
}
