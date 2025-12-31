import "../message"
import "../out_actions"
import "../../emulation/network"
import "../../testing/expect"
import "../../testing/outlist_expect"
import "../../vm/vm"

struct (0x7e8764ef) IncreaseCounter {
    queryId: uint64
    increaseBy: uint32
}

get fun `test-load-opcode`() {
    val dest = net.randomAddress("counter");
    val msg = createMessage({
        bounce: false,
        value: ton("1.9999"),
        dest,
        body: IncreaseCounter { queryId: 0, increaseBy: 1 },
    });
    msg.send(SEND_MODE_REGULAR | SEND_MODE_BOUNCE_ON_ACTION_FAIL);
    val out_actions = vm.outActions();
    expect(out_actions).toBeSendMessageAt<IncreaseCounter>(0);

    val action = out_actions.at(0);
    if (action is OutActionSendMessage) {
        val message = action.loadGenericMessage();
        val opcode = message.loadOpcode();
        expect(opcode).toEqual(IncreaseCounter.getDeclaredPackPrefix());
    }
}

get fun `test-load-opcode-for-bounced-message`() {
    val dest = net.randomAddress("counter");
    val msg = createMessage({
        bounce: false,
        value: ton("1.9999"),
        dest: dest,
        body: beginCell().storeUint(0xFFFFFFFF, 32).storeUint(IncreaseCounter.getDeclaredPackPrefix(), 32).endCell(),
    });
    msg.send(SEND_MODE_REGULAR | SEND_MODE_BOUNCE_ON_ACTION_FAIL);
    val out_actions = vm.outActions();
    expect(out_actions).toBeSendMessageAt<IncreaseCounter>(0);

    val action = out_actions.at(0);
    if (action is OutActionSendMessage) {
        val message = action.loadGenericMessage();
        val opcode = message.loadOpcode(false);
        expect(opcode).toEqual(0xFFFFFFFF);

        val actual_opcode = message.loadOpcode(true);
        expect(actual_opcode).toEqual(IncreaseCounter.getDeclaredPackPrefix());
    }
}
