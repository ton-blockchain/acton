import "@acton/emulation/network"
import "@acton/emulation/testing"
import "@acton/testing/expect"

import "@contracts/types"
import "@wrappers/Empty"

get fun `test deploy exposes initial owner`() {
    val (contract, deployer, _) = setupTest();
    expect(contract.owner()).toEqual(deployer.address);
}

get fun `test owner can transfer ownership`() {
    val (contract, deployer, notDeployer) = setupTest();

    val res = contract.sendChangeOwner(
        deployer.address,
        notDeployer.address,
        { value: ton("0.05") },
    );
    expect(res).toHaveSuccessfulTx<ChangeOwner>({
        from: deployer.address,
        to: contract.address,
    });

    expect(contract.owner()).toEqual(notDeployer.address);
}

get fun `test non-owner cannot transfer ownership`() {
    val (contract, deployer, notDeployer) = setupTest();

    val res = contract.sendChangeOwner(
        notDeployer.address,
        notDeployer.address,
        { value: ton("0.05") },
    );
    expect(res).toHaveFailedTx({
        from: notDeployer.address,
        to: contract.address,
        exitCode: Errors.NotOwner as int,
    });

    expect(contract.owner()).toEqual(deployer.address);
}

get fun `test unknown message fails with named error`() {
    val (contract, deployer, _) = setupTest();

    val res = contract.sendAny(
        deployer.address,
        beginCell().storeUint(0x999, 32).endCell(),
        { value: ton("0.05") },
    );
    expect(res).toHaveFailedTx({
        from: deployer.address,
        to: contract.address,
        exitCode: Errors.InvalidMessage as int,
    });

    val res2 = contract.sendAny(deployer.address, createEmptyCell(), { value: ton("0.05") });
    expect(res2).toHaveSuccessfulTx({
        from: deployer.address,
        to: contract.address,
    });
}

fun setupTest(): (Empty, Treasury, Treasury) {
    val deployer = testing.treasury("deployer");
    val notDeployer = testing.treasury("not_deployer");

    val contract = Empty.fromStorage(Storage { owner: deployer.address });
    val res = contract.deploy(deployer.address, { value: ton("0.05") });
    expect(res).toHaveSuccessfulDeploy({
        to: contract.address,
    });

    return (contract, deployer, notDeployer);
}
