import "@local-acton/emulation/network"
import "@local-acton/build/build"
import "@local-acton/testing/expect"
import "@local-acton/testing/transaction_expect"
import "contracts/types"

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
    val res = net.send(deployer.address, msg);
    expect(res).toHaveSuccessfulDeploy({ to: counter.address });

    return (counter, deployer)
}

fun Counter.getCounter(self): int {
    return net.runGetMethod(self.address, "currentCounter")
}

get fun `test-run-get-method-with-int-return-type`() {
    val (counter, deployer) = setupTest();

    val res = counter.sendIncrease(deployer.address, 100);
    expect(res).toHaveSuccessfulTx({ from: deployer.address, to: counter.address });

    expect(counter.getCounter()).toEqual(100);
}

get fun `test-run-get-method-with-int-int-return-type`() {
    val (counter, deployer) = setupTest();

    val res = counter.sendIncrease(deployer.address, 100);
    expect(res).toHaveSuccessfulTx({ from: deployer.address, to: counter.address });

    val result: tuple = net.runGetMethod(counter.address, "currentCounterDoubled");
    expect(result).toEqual([100, 200]);
}

get fun `test-run-get-method-with-nullable-int-return-type`() {
    val (counter, deployer) = setupTest();

    val res = counter.sendIncrease(deployer.address, 100);
    expect(res).toHaveSuccessfulTx({ from: deployer.address, to: counter.address });

    val result: int? = net.runGetMethod(counter.address, "currentCounterIfGraterThan100");
    expect(result).toEqual(null);

    val res2 = counter.sendIncrease(deployer.address, 200);
    expect(res2).toHaveSuccessfulTx({ from: deployer.address, to: counter.address });

    val result2: int? = net.runGetMethod(counter.address, "currentCounterIfGraterThan100");
    expect(result2).toEqual(30000);
}

get fun `test-run-get-method-with-struct-with-two-int-return-type`() {
    val (counter, deployer) = setupTest();

    val res = counter.sendIncrease(deployer.address, 100);
    expect(res).toHaveSuccessfulTx({ from: deployer.address, to: counter.address });

    val result: tuple = net.runGetMethod(counter.address, "currentStorage");
    expect(result).toEqual([0, 100]);
}

get fun `test-run-get-method-with-nullable-struct-with-two-int-return-type`() {
    val (counter, deployer) = setupTest();

    val res = counter.sendIncrease(deployer.address, 100);
    expect(res).toHaveSuccessfulTx({ from: deployer.address, to: counter.address });

    val result: tuple = net.runGetMethod(counter.address, "currentStorageOrNull");
    expect(result).toEqual([null, null, 0]);

    val res2 = counter.sendIncrease(deployer.address, 100);
    expect(res2).toHaveSuccessfulTx({ from: deployer.address, to: counter.address });

    val result2: tuple = net.runGetMethod(counter.address, "currentStorageOrNull");
    expect([result2.get(0) as int, result2.get(1) as int]).toEqual([0, 200]);
}

get fun `test-run-get-method-with-single-int-argument`() {
    val (counter, deployer) = setupTest();

    val res = counter.sendIncrease(deployer.address, 100);
    expect(res).toHaveSuccessfulTx({ from: deployer.address, to: counter.address });

    val result: int = net.runGetMethod(counter.address, "double", 999);
    expect(result).toEqual(2098);

    val result2: int = net.runGetMethod(counter.address, "double", [999]);
    expect(result2).toEqual(2098);
}

get fun `test-run-get-method-with-two-int-argument`() {
    val (counter, deployer) = setupTest();

    val res = counter.sendIncrease(deployer.address, 100);
    expect(res).toHaveSuccessfulTx({ from: deployer.address, to: counter.address });

    val result: int = net.runGetMethod(counter.address, "doubleBoth", [999, 100000]);
    expect(result).toEqual(102098);
}

get fun `test-run-get-method-with-objectt-argument`() {
    val (counter, deployer) = setupTest();

    val res = counter.sendIncrease(deployer.address, 100);
    expect(res).toHaveSuccessfulTx({ from: deployer.address, to: counter.address });

    val result: Pair = net.runGetMethod(counter.address, "doubleObject", Pair { x: 100, y: 200 });
    expect(result).toEqual(Pair { x: 200, y: 400 });
}

get fun `test-run-get-method-with-int-return-type-by-id`() {
    val (counter, deployer) = setupTest();

    val res = counter.sendIncrease(deployer.address, 100);
    expect(res).toHaveSuccessfulTx({ from: deployer.address, to: counter.address });

    val result: tuple = net.runGetMethodById(counter.address, 0x1cad0);
    expect(result).toEqual(100);
}
