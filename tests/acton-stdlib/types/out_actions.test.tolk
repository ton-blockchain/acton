import "@local-acton/emulation/network"
import "@local-acton/testing/expect"
import "@local-acton/testing/outlist_expect"
import "@local-acton/types/message"
import "@local-acton/types/out_actions"
import "@local-acton/vm/vm"

struct (0x7e8764ef) IncreaseCounter {
    queryId: uint64
    increaseBy: uint32
}

fun changeLib(code: cell, mode: int): void
    asm "SETLIBCODE"

/// Tests that we can actually retrieve out actions and parse it correctly
get fun `test-out-actions`() {
    val dest = net.randomAddress("counter");
    val msg = createMessage({
        bounce: false,
        value: ton("1.9999"),
        dest,
        body: IncreaseCounter {
            queryId: 0,
            increaseBy: 1,
        },
    });

    // Setup actions to test
    val new_code = beginCell().storeUint(0xFF, 8).endCell();
    contract.setCodePostponed(new_code);
    msg.send(SEND_MODE_REGULAR | SEND_MODE_BOUNCE_ON_ACTION_FAIL);
    reserveToncoinsOnBalance(
        ton("0.005"),
        RESERVE_MODE_ALL_BUT_AMOUNT | RESERVE_MODE_BOUNCE_ON_ACTION_FAIL,
    );
    changeLib(createEmptyCell(), 2);

    val out_actions = vm.outActions();

    val first_action = out_actions.at(0);
    expect(first_action.kind()).toEqual("change-library");
    if (first_action is OutActionChangeLibrary) {
        expect(first_action.mode).toEqual(2);
    }

    val second_action = out_actions.at(1);
    expect(second_action.kind()).toEqual("reserve-currency");
    if (second_action is OutActionReserveCurrency) {
        expect(second_action.mode).toEqual(
            RESERVE_MODE_ALL_BUT_AMOUNT | RESERVE_MODE_BOUNCE_ON_ACTION_FAIL,
        );
        expect(second_action.currency.grams).toEqual(ton("0.005"));
    }

    val third_action = out_actions.at(2);
    expect(third_action.kind()).toEqual("send-message");
    if (third_action is OutActionSendMessage) {
        expect(third_action.mode).toEqual(SEND_MODE_REGULAR | SEND_MODE_BOUNCE_ON_ACTION_FAIL);
        expect(out_actions).toBeSendMessageAt<IncreaseCounter>(2);

        val message = third_action.loadMessage<IncreaseCounter>();
        if (message.info is IntMsgInfoRelaxed) {
            expect(message.info.value.grams).toEqual(ton("1.9999"));
            expect(message.info.dest).toEqual(dest);
        }
    }

    val fourth_action = out_actions.at(3);
    expect(fourth_action.kind()).toEqual("set-code");
    if (fourth_action is OutActionSetCode) {
        expect(fourth_action.newCode).toEqual(new_code);
    }
}

get fun `test-out-message-out-actions-helper`() {
    val dest = net.randomAddress("counter");
    val msg = createMessage({
        bounce: false,
        value: ton("0.5"),
        dest,
        body: IncreaseCounter {
            queryId: 0,
            increaseBy: 7,
        },
    });

    val actions = msg.outActions(SEND_MODE_REGULAR);
    expect(actions).toHaveLength(1);
    expect(actions).toBeSendMessageAt<IncreaseCounter>(0);

    val action = actions.getSendMessageAt(0);
    expect(action).toBeNotNull();
    if (action != null) {
        expect(action.mode).toEqual(SEND_MODE_REGULAR);
    }

    val body = actions.getSendMessageBodyAt<IncreaseCounter>(0);
    expect(body).toBeNotNull();
    if (body != null) {
        expect(body.queryId).toEqual(0);
        expect(body.increaseBy).toEqual(7);
    }
}

get fun `test-parse-empty-out-actions`() {
    val actions = vm.parseOutActions(createEmptyCell());
    expect(actions).toHaveLength(0);
}

get fun `test-get-send-message-helpers-return-null-for-non-send-action`() {
    val dest = net.randomAddress("counter");
    val msg = createMessage({
        bounce: false,
        value: ton("1"),
        dest,
        body: IncreaseCounter {
            queryId: 0,
            increaseBy: 1,
        },
    });

    contract.setCodePostponed(beginCell().storeUint(0xEE, 8).endCell());
    msg.send(SEND_MODE_REGULAR | SEND_MODE_BOUNCE_ON_ACTION_FAIL);
    reserveToncoinsOnBalance(
        ton("0.005"),
        RESERVE_MODE_ALL_BUT_AMOUNT | RESERVE_MODE_BOUNCE_ON_ACTION_FAIL,
    );
    changeLib(createEmptyCell(), 2);

    val actions = vm.outActions();
    expect(actions.getSendMessageAt(0)).toBeNull();
    expect(actions.getSendMessageBodyAt<IncreaseCounter>(0)).toBeNull();
}
