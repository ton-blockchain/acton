import "../contracts/messages"

import "../../../../lib/build"
import "../../../../lib/emulation/network"
import "../../../../lib/emulation/testing"
import "../../../../lib/testing/expect"

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
    return Counter { address, init };
}

fun setupTest(): (Counter, Treasury) {
    val counter = Counter.fromStorage({});

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

get fun `test should setup test`() {
    setupTest();
}
