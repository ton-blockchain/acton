import "../message"
import "contracts/types"
import "../../build/build"
import "../../emulation/network"
import "../../testing/expect"
import "../../testing/transaction_expect"

struct Counter {
    address: address
    init: ContractState
}

fun Counter.fromStorage(storage: Storage) {
    val init = ContractState {
        code: build("Counter", "lib/types/tests/contracts/counter.tolk"),
        data: storage.toCell(),
    };
    val address = AutoDeployAddress { stateInit: init }.calculateAddress();
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

get fun `test-externals`() {
    val (counter, deployer) = setupTest();

    val res = counter.sendIncrease(deployer.address, 100);
    expect(res).toHaveSuccessfulTx({ from: deployer.address, to: counter.address });

    val tx = res.at(0);
    expect(tx.externals).toHaveLength(2);

    val first = tx.externals.at<IncreaseCounter>(0);
    expect(first.loadBody()).toEqual(IncreaseCounter { increaseBy: 100 });
    expect(first.info.dest).toEqual(createAddressNone());

    val second = tx.externals.at<ResetCounter>(1);
    expect(second.loadBody()).toEqual(ResetCounter {});

    val addr = address.fromValidBuilder(beginCell().storeUint(0b01, 2).storeUint(16, 9).storeUint(0xDEAD, 16));
    expect(second.info.dest).toEqual(addr);
}
