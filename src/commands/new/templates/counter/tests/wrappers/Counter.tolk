import "../../.acton/build/build"
import "../../.acton/emulation/network"
import "../../.acton/testing/expect"
import "../../.acton/types/message"
import "../../contracts/types"
import "../../contracts/types"

/// Configuration for sending messages to the contract
struct CounterSendMessageConfig {
    /// The amount of TON to send with the message (default: 0.1 TON)
    value: coins = ton("0.1")
    /// Whether to bounce the message if the contract does not exist or fails (default: false)
    bounce: bool = false
}

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
fun Counter.deploy(self, from: address, config: CounterSendMessageConfig = {}): SendResultList {
    val msg = createMessage({
        bounce: config.bounce,
        value: config.value,
        dest: {
            stateInit: self.init,
        },
    });
    return net.send(from, msg, SEND_MODE_PAY_FEES_SEPARATELY)
}

fun Counter.sendIncreaseCounter(self, from: address, increaseBy: uint32, config: CounterSendMessageConfig = {}): SendResultList {
    val msg = createMessage({
        bounce: config.bounce,
        value: config.value,
        dest: self.address,
        body: IncreaseCounter {
            increaseBy,
        },
    });
    return net.send(from, msg, SEND_MODE_PAY_FEES_SEPARATELY)
}

fun Counter.sendResetCounter(self, from: address, config: CounterSendMessageConfig = {}): SendResultList {
    val msg = createMessage({
        bounce: config.bounce,
        value: config.value,
        dest: self.address,
        body: ResetCounter {},
    });
    return net.send(from, msg, SEND_MODE_PAY_FEES_SEPARATELY)
}

fun Counter.sendAny(self, from: address, body: cell, config: CounterSendMessageConfig = {}): SendResultList {
    val msg = createMessage({
        bounce: config.bounce,
        value: config.value,
        dest: self.address,
        body,
    });
    return net.send(from, msg, SEND_MODE_PAY_FEES_SEPARATELY)
}

fun Counter.currentCounter(self): int {
    return net.runGetMethod(self.address, "currentCounter")
}
