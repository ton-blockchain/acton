import "@acton/build"
import "@acton/emulation/network"
import "@acton/emulation/testing"
import "@acton/testing/expect"
import "@acton/types/big_array"
import "../contracts/ext-out-demo"

get fun `test inspect external-out messages`() {
    val sender = testing.treasury("Sender");
    val stateInit = ContractState {
        code: build("ExtOutDemo"),
        data: createEmptyCell(),
    };
    val result = net.send(
        sender.address,
        createMessage({
            bounce: false,
            value: ton("2"),
            dest: { stateInit },
            body: Trigger { queryId: 42 },
        }),
    );
    result.giveName("Two external-out messages");
    expect(result).toHaveLength(2);
    expect(result.at(0).externals).toHaveLength(2);
}
