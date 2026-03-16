import "@acton/emulation/network"
import "@acton/testing/expect"
import "@acton/testing/transaction_expect"

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
    expect(res).toHaveSuccessfulTx<ChangeOwner>({ from: deployer.address, to: contract.address });

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

fun setupTest(): (Empty, Treasury, Treasury) {
    val deployer = net.treasury("deployer");
    val notDeployer = net.treasury("not_deployer");

    val contract = Empty.fromStorage(Storage { owner: deployer.address });
    val res = contract.deploy(deployer.address, { value: ton("0.05") });
    expect(res).toHaveSuccessfulDeploy({ to: contract.address });

    return (contract, deployer, notDeployer);
}
