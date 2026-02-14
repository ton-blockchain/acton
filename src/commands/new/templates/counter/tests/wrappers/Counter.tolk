import "@acton/build/build"
import "@acton/emulation/network"
import "@contracts/types"

struct Counter {
    address: address
    init: ContractState
}

/// Creates a contract wrapper instance from the storage data
fun Counter.fromStorage(storage: Storage) {
    val init = ContractState {
        code: build("counter"),
        data: storage.toCell(),
    };
    val address = AutoDeployAddress { stateInit: init }.calculateAddress();
    return Counter { address, init }
}

/// Deploys the contract to the blockchain
fun Counter.deploy(self, from: address, config: SendParams = {}): SendResultList {
    val deployMsg = createMessage({
        bounce: config.bounce,
        value: config.value,
        dest: {
            stateInit: self.init,
        },
    });
    return net.send(from, deployMsg, SEND_MODE_PAY_FEES_SEPARATELY)
}

fun Counter.sendIncreaseCounter(self, from: address, increaseBy: uint32, config: SendParams = {}): SendResultList {
    val increaseMsg = createMessage({
        bounce: config.bounce,
        value: config.value,
        dest: self.address,
        body: IncreaseCounter {
            increaseBy,
        },
    });
    return net.send(from, increaseMsg, SEND_MODE_PAY_FEES_SEPARATELY)
}

fun Counter.sendResetCounter(self, from: address, config: SendParams = {}): SendResultList {
    val resetMsg = createMessage({
        bounce: config.bounce,
        value: config.value,
        dest: self.address,
        body: ResetCounter {},
    });
    return net.send(from, resetMsg, SEND_MODE_PAY_FEES_SEPARATELY)
}

fun Counter.sendAny(self, from: address, body: cell, config: SendParams = {}): SendResultList {
    val anyMsg = createMessage({
        bounce: config.bounce,
        value: config.value,
        dest: self.address,
        body,
    });
    return net.send(from, anyMsg, SEND_MODE_PAY_FEES_SEPARATELY)
}

fun Counter.currentCounter(self): int {
    return net.runGetMethod(self.address, "currentCounter")
}
