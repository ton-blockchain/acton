import "@acton/emulation/network"
import "@acton/emulation/testing"
import "@acton/testing/expect"

import "@contracts/types"
import "@wrappers/Counter"

get fun `test deploy starts at zero`() {
    val (contract, _, _) = setupTest();
    expect(contract.currentCounter()).toEqual(0);
}

get fun `test increase counter`() {
    val (contract, deployer, _) = setupTest();

    val res = contract.sendIncreaseCounter(deployer.address, 123);
    expect(res).toHaveSuccessfulTx<IncreaseCounter>({
        from: deployer.address,
        to: contract.address,
    });

    expect(contract.currentCounter()).toEqual(123);
}

get fun `test any account can increase counter`() {
    val (contract, _, notDeployer) = setupTest();

    val res = contract.sendIncreaseCounter(notDeployer.address, 7);
    expect(res).toHaveSuccessfulTx<IncreaseCounter>({
        from: notDeployer.address,
        to: contract.address,
    });

    expect(contract.currentCounter()).toEqual(7);
}

get fun `test reset counter`() {
    val (contract, deployer, _) = setupTest();

    val res = contract.sendIncreaseCounter(deployer.address, 123);
    expect(res).toHaveSuccessfulTx<IncreaseCounter>({
        from: deployer.address,
        to: contract.address,
    });

    expect(contract.currentCounter()).toEqual(123);

    val res2 = contract.sendResetCounter(deployer.address);
    expect(res2).toHaveSuccessfulTx<ResetCounter>({ from: deployer.address, to: contract.address });

    expect(contract.currentCounter()).toEqual(0);
}

get fun `test unknown message`() {
    val (contract, deployer, _) = setupTest();

    // non-empty message
    val res = contract.sendAny(deployer.address, beginCell().storeInt(0x999, 32).endCell());
    expect(res).toHaveFailedTx({
        from: deployer.address,
        to: contract.address,
        exitCode: Errors.InvalidMessage as int,
    });

    // empty message
    val res2 = contract.sendAny(deployer.address, createEmptyCell());
    expect(res2).toHaveSuccessfulTx({ from: deployer.address, to: contract.address });
}

/// Initializes the test environment, creating a fresh instance of the contract.
/// Returns the contract wrapper and two treasury accounts (`deployer` and `not_deployer`).
fun setupTest(): (Counter, Treasury, Treasury) {
    // Create a treasury account for deployment (typically the owner)
    val deployer = testing.treasury("deployer");
    // Create another treasury account for testing interactions from other users
    val notDeployer = testing.treasury("not_deployer");

    // Initialize and deploy the contract with default values
    val contract = Counter.fromStorage({ id: 0, counter: 0 });
    val res = contract.deploy(deployer.address, { value: ton("1") });
    expect(res).toHaveSuccessfulDeploy({ to: contract.address });

    return (contract, deployer, notDeployer);
}
