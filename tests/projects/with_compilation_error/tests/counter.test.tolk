import "../../../../lib/build/build"
import "../../../../lib/emulation/network"
import "../../../../lib/testing/expect"
import "../../../../lib/testing/transaction_expect"

import "../contracts/messages"

struct Counter {
    address: address
    init: ContractState
}

fun Counter.fromStorage(storage: Storage) {
    val init = ContractState {
        code: build("counter"),
        data: storage.toCell(),
    };
    val address = AutoDeployAddress { stateInit: init }.calculateAddress();
    return Counter { address, init }
}

fun setupTest() {
    val counter = Counter.fromStorage({});

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

get fun `test-should-setup-test`() {
    setupTest();
}
