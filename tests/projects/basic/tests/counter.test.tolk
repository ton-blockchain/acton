import "../../../../lib/build/build"
import "../../../../lib/emulation/network"
import "../../../../lib/testing/expect"
import "../../../../lib/testing/transaction_expect"
import "../../../../lib/io"

import "../contracts/counter_messages"

struct Counter {
    address: address
    init: ContractState
}

fun Counter.fromStorage(storage: Storage): Counter {
    val init = ContractState {
        code: build("counter"),
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
        body: IncreaseCounter { queryId: 0, increaseBy },
    });
    return net.send(from, msg);
}

fun Counter.sendReset(self, from: address): SendResultList {
    val msg = createMessage({
        bounce: false,
        value: ton("0.1"),
        dest: self.address,
        body: ResetCounter { queryId: 0 },
    });
    return net.send(from, msg);
}

fun Counter.getCounter(self): int {
    return net.runGetMethod(self.address, "currentCounter")
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

// SLOT_1: @test({gas_limit: 100})
get fun `test-should-increase-counter`() {
    val (counter, deployer) = setupTest();

    val res = counter.sendIncrease(deployer.address, 100);
    expect(res).toHaveSuccessfulTx({ from: deployer.address, to: counter.address });
    expect(counter.getCounter()).toEqual(100);

    // SLOT_2: expect(1).toEqual(2);
    // SLOT_5: println(res)
    // SLOT_6: eprintln("error output")
}

// SLOT_3: @test({fail_with: 100})
get fun `test-should-reset-counter`() {
    // SLOT_4: if (true) { throw 9 }
    val (counter, deployer) = setupTest();

    val res = counter.sendIncrease(deployer.address, 100);
    expect(res).toHaveSuccessfulTx({ from: deployer.address, to: counter.address });
    expect(counter.getCounter()).toEqual(100);

    val resetRes = counter.sendReset(deployer.address);
    expect(resetRes).toHaveSuccessfulTx({ from: deployer.address, to: counter.address });
    expect(counter.getCounter()).toEqual(0);
}
