import "types"
import "lib/build/build"
import "lib/emulation/network"
import "lib/testing/expect"
import "lib/testing/transaction_expect"

struct Counter {
    address: address
    init: ContractState
}

fun Counter.fromStorage(storage: Storage) {
    val init = ContractState {
        code: build("Counter", "counter2.tolk"),
        data: storage.toCell(),
    };
    val address = address.fromValidBuilder(AutoDeployAddress { stateInit: init }.buildAddress());
    return Counter { address, init }
}

fun Counter.sendIncrease(self, from: address, increaseBy: int) {
    val msg = createMessage({
        bounce: false,
        value: ton("0.1"),
        dest: self.address,
        body: IncreaseCounter { increaseBy },
    });
    return net.send(from, msg, SEND_MODE_PAY_FEES_SEPARATELY);
}

fun Counter.sendReset(self, from: address) {
    val msg = createMessage({
        bounce: false,
        value: ton("0.1"),
        dest: self.address,
        body: ResetCounter {},
    });
    return net.send(from, msg, SEND_MODE_PAY_FEES_SEPARATELY);
}

fun Counter.getCounter(self): int {
    return net.runGetMethod(self.address, "currentCounter")
}

fun setupTest() {
    val counter = Counter.fromStorage({ id: 0, counter: 0 });

    val deployer = net.treasury("deployer");
    val msg = createMessage({
        bounce: false,
        value: ton("1.0"),
        dest: {
            stateInit: counter.init,
        },
    });
    val res = net.send(deployer.address, msg, SEND_MODE_PAY_FEES_SEPARATELY);
    expect(res).toHaveSuccessfulDeploy({ to: counter.address });

    return (counter, deployer)
}

get fun `test-should-deploy`() {
    setupTest()
}

get fun `test-should-increase-counter`() {
    val (counter, deployer) = setupTest();

    val res = counter.sendIncrease(deployer.address, 100);
    expect(res).toHaveSuccessfulTx({ from: deployer.address, to: counter.address });

    expect(counter.getCounter()).toEqual(100);
}

get fun `test-should-reset-counter`() {
    val (counter, deployer) = setupTest();

    val res = counter.sendIncrease(deployer.address, 100);
    expect(res).toHaveSuccessfulTx({ from: deployer.address, to: counter.address });
    expect(counter.getCounter()).toEqual(100);

    val resetRes = counter.sendReset(deployer.address);
    expect(resetRes).toHaveSuccessfulTx({ from: deployer.address, to: counter.address });
    expect(counter.getCounter()).toEqual(0);
}
