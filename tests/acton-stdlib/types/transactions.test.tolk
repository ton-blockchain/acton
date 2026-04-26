import "@local-acton/build"
import "@local-acton/emulation/network"
import "@local-acton/emulation/testing"
import "@local-acton/testing/expect"
import "@local-acton/types/message"
import "@local-acton/types/transaction"
import "contracts/types"

struct Counter {
    address: address
    init: ContractState
}

fun Counter.fromStorage(storage: Storage): Counter {
    val init = ContractState {
        code: build("Counter", "tests/acton-stdlib/types/contracts/counter.tolk"),
        data: storage.toCell(),
    };
    val address = AutoDeployAddress { stateInit: init }.calculateAddress();
    return Counter { address, init };
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

    val deployer = testing.treasury("deployer");
    val msg = createMessage({
        bounce: false,
        value: ton("1.0"),
        dest: { stateInit: counter.init },
    });
    val res = net.send(deployer.address, msg);
    expect(res).toHaveSuccessfulDeploy({ to: counter.address });

    return (counter, deployer);
}

get fun `test load transaction body`() {
    val (counter, deployer) = setupTest();

    val res = counter.sendIncrease(deployer.address, 100);
    expect(res).toHaveSuccessfulTx({ from: deployer.address, to: counter.address });

    val tx = res.at(0).tx.load();
    val body = tx.loadBody<IncreaseCounter>();
    expect(body).toEqual(IncreaseCounter { increaseBy: 100 });
}

@test.fail_with(63)
get fun `test fail to load transaction body`() {
    val (counter, deployer) = setupTest();

    val res = counter.sendIncrease(deployer.address, 100);
    expect(res).toHaveSuccessfulTx({ from: deployer.address, to: counter.address });

    val tx = res.at(0).tx.load();
    val body = tx.loadBody<ResetCounter>();
    expect(body).toEqual(ResetCounter {});
}

get fun `test load transaction body after find transaction`() {
    val (counter, deployer) = setupTest();

    val res = counter.sendIncrease(deployer.address, 100);
    expect(res).toHaveSuccessfulTx({ from: deployer.address, to: counter.address });

    val tx = res.findTransaction<IncreaseCounter>()!;
    val body = tx.loadBody<IncreaseCounter>();
    expect(body).toEqual(IncreaseCounter { increaseBy: 100 });
}

@test.fail_with(63)
get fun `test fail to load transaction body after find transaction`() {
    val (counter, deployer) = setupTest();

    val res = counter.sendIncrease(deployer.address, 100);
    expect(res).toHaveSuccessfulTx({ from: deployer.address, to: counter.address });

    val tx = res.findTransaction<IncreaseCounter>()!;
    val body = tx.loadBody<ResetCounter>();
    expect(body).toEqual(ResetCounter {});
}

get fun `test load transaction in message`() {
    val (counter, deployer) = setupTest();

    val res = counter.sendIncrease(deployer.address, 100);
    expect(res).toHaveSuccessfulTx({ from: deployer.address, to: counter.address });

    val tx = res.at(0).tx.load();
    val inMsg = tx.loadInMsg<IncreaseCounter>();
    expect(inMsg.loadBody()).toEqual(IncreaseCounter { increaseBy: 100 });
}

get fun `test transaction helpers`() {
    val (counter, deployer) = setupTest();

    val res = counter.sendIncrease(deployer.address, 100);
    expect(res).toHaveSuccessfulTx({ from: deployer.address, to: counter.address });

    val first = res.at(0);
    val tx = first.tx.load();

    expect(tx.getUsedGas()).toEqual(first.gasUsed);
    expect(tx.getActionFee()).toBeNotNull();
    expect(tx.getActionFee()!).toBeGreater(0);
    expect(tx.getAccountAddress()).toEqual(counter.address);
}
