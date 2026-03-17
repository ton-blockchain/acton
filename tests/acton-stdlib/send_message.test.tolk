import "@local-acton/emulation/network"
import "@local-acton/build/build"
import "@local-acton/testing/expect"
import "@local-acton/testing/transaction_expect"
import "contracts/types"

struct (0x12345678) UnknownMessage {}

struct Counter {
    address: address
    init: ContractState
}

fun Counter.fromStorage(storage: Storage): Counter {
    val init = ContractState {
        code: build("Counter", "tests/acton-stdlib/contracts/counter.tolk"),
        data: storage.toCell(),
    };
    val address = AutoDeployAddress { stateInit: init }.calculateAddress();
    return Counter { address, init }
}

fun Counter.sendIncrease(self, from: address, increaseBy: int): SendResultList {
    val msg = createMessage({
        bounce: false,
        value: ton("0.1"),
        dest: self.address,
        body: IncreaseCounter { increaseBy },
    });
    return net.send(from, msg);
}

fun setupTest(): (Counter, Treasury) {
    val counter = Counter.fromStorage({ id: 0, counter: 0 });

    val deployer = net.treasury("deployer");
    val msg = createMessage({
        bounce: false,
        value: ton("1.0"),
        dest: {
            stateInit: counter.init,
        },
    });
    val _res = net.send(deployer.address, msg);
    // expect(res).toHaveSuccessfulDeploy({ to: counter.address });

    return (counter, deployer)
}

fun Counter.getCounter(self): int {
    return net.runGetMethod(self.address, "currentCounter")
}

get fun `test-send-message`() {
    val (counter, deployer) = setupTest();

    val res = counter.sendIncrease(deployer.address, 100);
    expect(res).toHaveSuccessfulTx({ from: deployer.address, to: counter.address });

    expect(counter.getCounter()).toEqual(100);
}

get fun `test-send-message-with-gas`() {
    val (counter, deployer) = setupTest();

    val res = counter.sendIncrease(deployer.address, 100);
    expect(res).toHaveSuccessfulTx({ from: deployer.address, to: counter.address });

    expect(counter.getCounter()).toEqual(100);

    expect(res.at(0)).toConsumeLessThan(1500);
}

get fun `test-find-transaction`() {
    val (counter, deployer) = setupTest();

    val res = counter.sendIncrease(deployer.address, 100);
    expect(res).toHaveSuccessfulTx({ from: deployer.address, to: counter.address });

    expect(counter.getCounter()).toEqual(100);

    val tx = res.findTransaction({ to: counter.address });
    expect(tx).toBeDefined();

    val unknownTx = res.findTransaction({ to: deployer.address });
    expect(unknownTx).toBeNone();
}

get fun `test-find-transaction-with-type`() {
    val (counter, deployer) = setupTest();

    val res = counter.sendIncrease(deployer.address, 100);
    expect(res).toHaveSuccessfulTx({ from: deployer.address, to: counter.address });

    expect(counter.getCounter()).toEqual(100);

    val tx = res.findTransaction<IncreaseCounter>({ to: counter.address });
    expect(tx).toBeDefined();

    val unknownTx = res.findTransaction<ResetCounter>({ to: counter.address });
    expect(unknownTx).toBeNone();
}

get fun `test-search-by-value`() {
    val (counter, deployer) = setupTest();

    val res = counter.sendIncrease(deployer.address, 100);
    expect(res).toHaveSuccessfulTx({ to: counter.address, value: ton("0.1") });
}

get fun `test-search-by-value-mismatch`() {
    val (counter, deployer) = setupTest();

    val res = counter.sendIncrease(deployer.address, 100);
    expect(res).toNotHaveTx({ to: counter.address, value: ton("0.2") });
}

get fun `test-all-transactions-successful`() {
    val (counter, deployer) = setupTest();

    val res = counter.sendIncrease(deployer.address, 100);
    expect(res).toHaveAllSuccessfulTxs();
    expect(counter.getCounter()).toEqual(100);
}

get fun `test-failed-transaction`() {
    val (counter, deployer) = setupTest();

    val msg = createMessage({
        bounce: false,
        value: ton("0.1"),
        dest: counter.address,
        body: UnknownMessage {},
    });
    val res = net.send(deployer.address, msg);
    expect(res).toHaveFailedTx({ from: deployer.address, to: counter.address, exitCode: 0xFFFF });
    expect(counter.getCounter()).toEqual(0);
}

get fun `test-bounced-transaction`() {
    val (counter, deployer) = setupTest();

    val msg = createMessage({
        bounce: false,
        value: ton("0.1"),
        dest: counter.address,
        body: IncreaseCounter { increaseBy: 100 },
    }).bounced();
    val res = net.send(deployer.address, msg);
    expect(res).toHaveBouncedTx({ to: counter.address });
    expect(counter.getCounter()).toEqual(0);
}
