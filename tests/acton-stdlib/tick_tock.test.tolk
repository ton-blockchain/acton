import "@local-acton/build"
import "@local-acton/emulation/network"
import "@local-acton/emulation/testing"
import "@local-acton/testing/expect"
import "@local-acton/types/big_array"
import "contracts/tick_tock_types"

struct TickTockContract {
    address: address
}

fun TickTockContract.deploy(): TickTockContract {
    val init = ContractState {
        code: build("TickTock", "tests/acton-stdlib/contracts/tick_tock.tolk"),
        data: TickTockStorage {
            tickCount: 0,
            tockCount: 0,
        }.toCell(),
    };
    val address = AutoDeployAddress { stateInit: init }.calculateAddress();

    val deployer = testing.treasury("deployer");
    net.send(
        deployer.address,
        createMessage({
            bounce: false,
            value: ton("1"),
            dest: { stateInit: init },
        }),
    );

    return TickTockContract { address };
}

fun TickTockContract.getTickCount(self): int {
    return net.runGetMethod(self.address, "tickCount");
}

fun TickTockContract.getTockCount(self): int {
    return net.runGetMethod(self.address, "tockCount");
}

get fun `test run tick`() {
    val contract = TickTockContract.deploy();

    val res = testing.runTickTock(contract.address, false);
    expect(res.size()).toBeGreater(0);

    expect(contract.getTickCount()).toEqual(1);
    expect(contract.getTockCount()).toEqual(0);
}

get fun `test run tock`() {
    val contract = TickTockContract.deploy();

    val res = testing.runTickTock(contract.address, true);
    expect(res.size()).toBeGreater(0);

    expect(contract.getTickCount()).toEqual(0);
    expect(contract.getTockCount()).toEqual(1);
}
