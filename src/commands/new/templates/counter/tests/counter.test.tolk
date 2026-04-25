import "@acton/emulation/network"
import "@acton/emulation/testing"
import "@acton/testing/expect"

import "@contracts/types"
import "@wrappers/Counter.gen"

get fun `test increase counter`() {
    val (contract, deployer, _) = setupTest();

    val res = contract.sendIncreaseCounter(deployer.address, 123);
    expect(res).toHaveSuccessfulTx<IncreaseCounter>({
        from: deployer.address,
        to: contract.address,
    });

    expect(contract.currentCounter()).toEqual(123);
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
    expect(res2).toHaveSuccessfulTx<ResetCounter>({
        from: deployer.address,
        to: contract.address,
    });

    expect(contract.currentCounter()).toEqual(0);
}

get fun `test decrease counter`() {
    val (contract, deployer, _) = setupTest();

    val res = contract.sendIncreaseCounter(deployer.address, 123);
    expect(res).toHaveSuccessfulTx<IncreaseCounter>({
        from: deployer.address,
        to: contract.address,
    });

    val res2 = contract.sendDecreaseCounter(deployer.address, 23);
    expect(res2).toHaveSuccessfulTx<DecreaseCounter>({
        from: deployer.address,
        to: contract.address,
    });

    expect(contract.currentCounter()).toEqual(100);
}

get fun `test decrease counter fails on underflow`() {
    val (contract, deployer, _) = setupTest();

    val res = contract.sendDecreaseCounter(deployer.address, 1);
    expect(res).toHaveFailedTx({
        from: deployer.address,
        to: contract.address,
        exitCode: 0x1001,
    });
}

get fun `test unknown message`() {
    val (contract, deployer, _) = setupTest();

    val res = contract.sendAny(deployer.address, beginCell().storeInt(0x999, 32).endCell());
    expect(res).toHaveFailedTx({
        from: deployer.address,
        to: contract.address,
        exitCode: 0xFFFF,
    });

    val res2 = contract.sendAny(deployer.address, createEmptyCell());
    expect(res2).toHaveSuccessfulTx({
        from: deployer.address,
        to: contract.address,
    });
}

fun setupTest(): (Counter, Treasury, Treasury) {
    val deployer = testing.treasury("deployer");
    val notDeployer = testing.treasury("not_deployer");

    val contract = Counter.fromStorage({ id: 0, counter: 0 });
    val res = contract.deploy(deployer.address, { value: ton("1") });
    expect(res).toHaveSuccessfulDeploy({
        to: contract.address,
    });

    return (contract, deployer, notDeployer);
}
