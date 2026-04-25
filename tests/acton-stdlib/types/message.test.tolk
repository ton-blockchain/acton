import "@stdlib/reflection"
import "@local-acton/emulation/network"
import "@local-acton/emulation/testing"
import "@local-acton/testing/expect"
import "@local-acton/tlb/either"
import "@local-acton/tlb/maybe"
import "@local-acton/types/message"
import "@local-acton/types/out_actions"

struct (0x7e8764ef) IncreaseCounter {
    queryId: uint64
    increaseBy: uint32
}

struct (0x11223344) DemoPayload {
    value: uint32
}

fun internalInfo(): TlbIntMsgInfoRelaxed {
    return TlbIntMsgInfoRelaxed {
        ihrDisabled: true,
        bounce: false,
        bounced: false,
        src: randomAddress("src") as any_address,
        dest: randomAddress("dest"),
        value: TlbCurrencyCollection {
            grams: 0,
            other: createEmptyMap<int32, varuint32>(),
        },
        extraFlags: 0,
        fwdFee: 0,
        createdLt: 0,
        createdAt: 0,
    };
}

fun externalInfo(): TlbExtMsgInfoRelaxed {
    return TlbExtMsgInfoRelaxed {
        src: randomAddress("src"),
        dest: createAddressNone(),
        createdLt: 0,
        createdAt: 0,
    };
}

get fun `test load opcode`() {
    val dest = randomAddress("counter");
    val msg = createMessage({
        bounce: false,
        value: ton("1.9999"),
        dest,
        body: IncreaseCounter {
            queryId: 0,
            increaseBy: 1,
        },
    });
    msg.send(SEND_MODE_REGULAR | SEND_MODE_BOUNCE_ON_ACTION_FAIL);
    val out_actions = testing.outActions();
    expect(out_actions).toBeSendMessageAt<IncreaseCounter>(0);

    val action = out_actions.at(0);
    if (action is TlbOutActionSendMessage) {
        val message = action.loadGenericMessage();
        val opcode = message.loadOpcode();
        expect(opcode).toEqual(reflect.serializationPrefixOf<IncreaseCounter>().0);
    }
}

get fun `test load opcode for bounced message`() {
    val dest = randomAddress("counter");
    val msg = createMessage({
        bounce: false,
        value: ton("1.9999"),
        dest,
        body: beginCell()
            .storeUint(0xFFFFFFFF, 32)
            .storeUint(reflect.serializationPrefixOf<IncreaseCounter>().0, 32)
            .endCell(),
    });
    msg.send(SEND_MODE_REGULAR | SEND_MODE_BOUNCE_ON_ACTION_FAIL);
    val out_actions = testing.outActions();
    expect(out_actions).toBeSendMessageAt<IncreaseCounter>(0);

    val action = out_actions.at(0);
    if (action is TlbOutActionSendMessage) {
        val message = action.loadGenericMessage();
        val opcode = message.loadOpcode(false);
        expect(opcode).toEqual(0xFFFFFFFF);

        val actual_opcode = message.loadOpcode(true);
        expect(actual_opcode).toEqual(reflect.serializationPrefixOf<IncreaseCounter>().0);
    }
}

get fun `test load opcode for new bounced message prefix`() {
    val dest = randomAddress("counter-new-bounce");
    val msg = createMessage({
        bounce: false,
        value: ton("1.9999"),
        dest,
        body: beginCell()
            .storeUint(0xFFFFFFFE, 32)
            .storeUint(reflect.serializationPrefixOf<IncreaseCounter>().0, 32)
            .endCell(),
    });
    msg.send(SEND_MODE_REGULAR | SEND_MODE_BOUNCE_ON_ACTION_FAIL);
    val out_actions = testing.outActions();
    expect(out_actions).toBeSendMessageAt<IncreaseCounter>(0);

    val action = out_actions.at(0);
    if (action is TlbOutActionSendMessage) {
        val message = action.loadGenericMessage();
        val opcode = message.loadOpcode(false);
        expect(opcode).toEqual(0xFFFFFFFE);

        val actual_opcode = message.loadOpcode(true);
        expect(actual_opcode).toEqual(reflect.serializationPrefixOf<IncreaseCounter>().0);
    }
}

get fun `test load opcode returns null for missing either tag`() {
    val msg = TlbMessageRelaxedGeneric {
        info: internalInfo(),
        init: TlbMaybe.none(),
        body: beginCell().endCell().beginParse(),
    };

    expect(msg.loadOpcode()).toBeNull();
}

get fun `test load body from inline and ref`() {
    val inlineMessage = TlbMessageRelaxed<DemoPayload> {
        info: internalInfo(),
        init: TlbMaybe.none(),
        body: TlbEither<DemoPayload, Cell<DemoPayload>>.left(DemoPayload { value: 1 }),
    };
    expect(inlineMessage.loadBody()).toEqual(DemoPayload { value: 1 });

    val refMessage = TlbMessageRelaxed<DemoPayload> {
        info: internalInfo(),
        init: TlbMaybe.none(),
        body: TlbEither<DemoPayload, Cell<DemoPayload>>.right(DemoPayload { value: 2 }.toCell()),
    };
    expect(refMessage.loadBody()).toEqual(DemoPayload { value: 2 });

    val extRefMessage = TlbExtMessageRelaxed<DemoPayload> {
        info: externalInfo(),
        init: TlbMaybe.none(),
        body: TlbEither<DemoPayload, Cell<DemoPayload>>.right(DemoPayload { value: 3 }.toCell()),
    };
    expect(extRefMessage.loadBody()).toEqual(DemoPayload { value: 3 });
}

get fun `test load opcode from inline and ref body`() {
    val opcode = reflect.serializationPrefixOf<IncreaseCounter>().0;

    val inlineMessage = TlbMessageRelaxedGeneric {
        info: internalInfo(),
        init: TlbMaybe.none(),
        body: beginCell().storeBool(false).storeUint(opcode, 32).endCell().beginParse(),
    };
    expect(inlineMessage.loadOpcode()).toEqual(opcode);

    val refMessage = TlbMessageRelaxedGeneric {
        info: internalInfo(),
        init: TlbMaybe.none(),
        body: beginCell()
            .storeBool(true)
            .storeRef(beginCell().storeUint(opcode, 32).endCell())
            .endCell()
            .beginParse(),
    };
    expect(refMessage.loadOpcode()).toEqual(opcode);

    val externalMessage = TlbExtMessageRelaxedGeneric {
        info: externalInfo(),
        init: TlbMaybe.none(),
        body: beginCell()
            .storeBool(true)
            .storeRef(beginCell().storeUint(opcode, 32).endCell())
            .endCell()
            .beginParse(),
    };
    expect(externalMessage.loadOpcode()).toEqual(opcode);
}

get fun `test load opcode returns null for malformed body`() {
    val missingRef = TlbMessageRelaxedGeneric {
        info: internalInfo(),
        init: TlbMaybe.none(),
        body: beginCell().storeBool(true).endCell().beginParse(),
    };
    expect(missingRef.loadOpcode()).toBeNull();

    val shortBody = TlbMessageRelaxedGeneric {
        info: internalInfo(),
        init: TlbMaybe.none(),
        body: beginCell().storeBool(false).storeUint(0b10101, 5).endCell().beginParse(),
    };
    expect(shortBody.loadOpcode()).toBeNull();

    val bouncedWithoutActualOpcode = TlbExtMessageRelaxedGeneric {
        info: externalInfo(),
        init: TlbMaybe.none(),
        body: beginCell().storeBool(false).storeUint(0xFFFFFFFF, 32).endCell().beginParse(),
    };
    expect(bouncedWithoutActualOpcode.loadOpcode(true)).toBeNull();
}
