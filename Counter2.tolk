import "@stdlib/gas-payments"
import "@acton/build/build"
import "@acton/emulation/network"
import "@acton/testing/assert"
import "@acton/testing/expect"
import "@acton/types/message"
import "types"

struct Counter {
    address: address
    stateInit: ContractState? = null
}

/// Creates a contract wrapper instance from the storage data
fun Counter.fromStorage(storage: Storage, toShard: AddressShardingOptions? = null) {
    val stateInit = ContractState {
        code: build("counter"),
        data: storage.toCell(),
    };
    val address = toShard == null
        ? AutoDeployAddress { stateInit }.calculateAddress()
        : AutoDeployAddress { stateInit, toShard }.calculateAddress();
    return Counter { address, stateInit }
}

/// Creates a contract wrapper instance from the address
fun Counter.fromAddress(address: address) {
    return Counter { address }
}

/// Deploys the contract to the blockchain
fun Counter.deploy(self, from: address, config: SendParams = {}): SendResultList {
    if (self.stateInit == null) {
        Assert.fail("Cannot deploy a contract created with 'fromAddress' because it lacks state init for deployment");
    }
    val msg = createMessage({
        bounce: config.bounce,
        value: config.value,
        dest: {
            stateInit: self.stateInit,
        },
    });
    return net.send(from, msg, SEND_MODE_PAY_FEES_SEPARATELY)
}

fun Counter.sendIncreaseCounter(self, from: address, increaseBy: uint32, config: SendParams = {}): SendResultList {
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

fun Counter.sendResetCounter(self, from: address, config: SendParams = {}): SendResultList {
    val msg = createMessage({
        bounce: config.bounce,
        value: config.value,
        dest: self.address,
        body: ResetCounter {},
    });
    return net.send(from, msg, SEND_MODE_PAY_FEES_SEPARATELY)
}

/// Send message to the contract with a custom body cell
fun Counter.sendAny(self, from: address, body: cell, config: SendParams = {}): SendResultList {
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
