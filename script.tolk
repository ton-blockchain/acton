import "types"
import "lib/io"
import "lib/build/build"
import "lib/emulation/network"
import "lib/promts/prompts"
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

@inline_ref
fun foo() {
    val counter = Counter.fromStorage({ id: 0, counter: 100 });

    val deployer = net.treasury("deployer");
    val msg = createMessage({
        bounce: false,
        value: ton("1.0"),
        dest: {
            stateInit: counter.init,
        },
    });
    val res = net.send(deployer.address, msg, SEND_MODE_PAY_FEES_SEPARATELY);

    {
        val increaseMsg = createMessage({
            bounce: false,
            value: ton("1.0"),
            dest: counter.address,
            body: IncreaseCounter {
                increaseBy: 99,
            },
        });
        val increaseRes = net.send(deployer.address, increaseMsg, SEND_MODE_PAY_FEES_SEPARATELY);
        println(increaseRes);
    }

    {
        val res22: int = net.runGetMethod(counter.address, "currentCounter");
        println(res22);
    }
    {
        val res22: int = net.runGetMethod(counter.address, "currentCounter");
        println(res22);
    }

    expect(res).toHaveSuccessfulTx({ to: counter.address });
}

fun main() {
    foo();
}
