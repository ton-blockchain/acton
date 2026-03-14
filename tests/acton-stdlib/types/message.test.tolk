import "@stdlib/reflection"
import "@local-acton/types/message"
import "@local-acton/types/out_actions"
import "@local-acton/emulation/network"
import "@local-acton/testing/expect"
import "@local-acton/testing/outlist_expect"
import "@local-acton/tlb/either"
import "@local-acton/tlb/maybe"
import "@local-acton/vm/vm"

struct (0x7e8764ef) IncreaseCounter {
    queryId: uint64
    increaseBy: uint32
}

struct (0x11223344) DemoPayload {
    value: uint32
}

fun internalInfo(): IntMsgInfoRelaxed {
    return IntMsgInfoRelaxed {
        ihrDisabled: true,
        bounce: false,
        bounced: false,
        src: net.randomAddress("src") as any_address,
        dest: net.randomAddress("dest"),
        value: CurrencyCollection {
            grams: 0,
            other: createEmptyMap<int32, varuint32>(),
        },
        extraFlags: 0,
        fwdFee: 0,
        createdLt: 0,
        createdAt: 0,
    };
}

fun externalInfo(): ExtMsgInfoRelaxed {
    return ExtMsgInfoRelaxed {
        src: net.randomAddress("src"),
        dest: createAddressNone(),
        createdLt: 0,
        createdAt: 0,
    };
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
        expect(opcode).toEqual(reflect.serializationPrefixOf<IncreaseCounter>());
    }
}

get fun `test-load-opcode-for-bounced-message`() {
    val dest = net.randomAddress("counter");
    val msg = createMessage({
        bounce: false,
        value: ton("1.9999"),
        dest: dest,
        body: beginCell().storeUint(0xFFFFFFFF, 32).storeUint(reflect.serializationPrefixOf<IncreaseCounter>(), 32).endCell(),
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
        expect(actual_opcode).toEqual(reflect.serializationPrefixOf<IncreaseCounter>());
    }
}

get fun `test-load-body-from-inline-and-ref`() {
    val inlineMessage = MessageRelaxed<DemoPayload> {
        info: internalInfo(),
        init: Maybe<Either<StateInit, Cell<StateInit>>>.none(),
        body: Either<DemoPayload, Cell<DemoPayload>>.left(DemoPayload { value: 1 }),
    };
    expect(inlineMessage.loadBody()).toEqual(DemoPayload { value: 1 });

    val refMessage = MessageRelaxed<DemoPayload> {
        info: internalInfo(),
        init: Maybe<Either<StateInit, Cell<StateInit>>>.none(),
        body: Either<DemoPayload, Cell<DemoPayload>>.right(DemoPayload { value: 2 }.toCell()),
    };
    expect(refMessage.loadBody()).toEqual(DemoPayload { value: 2 });

    val extRefMessage = ExtMessageRelaxed<DemoPayload> {
        info: externalInfo(),
        init: Maybe<Either<StateInit, Cell<StateInit>>>.none(),
        body: Either<DemoPayload, Cell<DemoPayload>>.right(DemoPayload { value: 3 }.toCell()),
    };
    expect(extRefMessage.loadBody()).toEqual(DemoPayload { value: 3 });
}

get fun `test-load-opcode-from-inline-and-ref-body`() {
    val opcode = reflect.serializationPrefixOf<IncreaseCounter>();

    val inlineMessage = MessageRelaxedGeneric {
        info: internalInfo(),
        init: Maybe<Either<StateInit, Cell<StateInit>>>.none(),
        body: beginCell().storeBool(false).storeUint(opcode, 32).endCell().beginParse(),
    };
    expect(inlineMessage.loadOpcode()).toEqual(opcode);

    val refMessage = MessageRelaxedGeneric {
        info: internalInfo(),
        init: Maybe<Either<StateInit, Cell<StateInit>>>.none(),
        body: beginCell()
            .storeBool(true)
            .storeRef(beginCell().storeUint(opcode, 32).endCell())
            .endCell()
            .beginParse(),
    };
    expect(refMessage.loadOpcode()).toEqual(opcode);

    val externalMessage = ExtMessageRelaxedGeneric {
        info: externalInfo(),
        init: Maybe<Either<StateInit, Cell<StateInit>>>.none(),
        body: beginCell()
            .storeBool(true)
            .storeRef(beginCell().storeUint(opcode, 32).endCell())
            .endCell()
            .beginParse(),
    };
    expect(externalMessage.loadOpcode()).toEqual(opcode);
}

get fun `test-load-opcode-returns-null-for-malformed-body`() {
    val missingRef = MessageRelaxedGeneric {
        info: internalInfo(),
        init: Maybe<Either<StateInit, Cell<StateInit>>>.none(),
        body: beginCell().storeBool(true).endCell().beginParse(),
    };
    expect(missingRef.loadOpcode()).toBeNull();

    val shortBody = MessageRelaxedGeneric {
        info: internalInfo(),
        init: Maybe<Either<StateInit, Cell<StateInit>>>.none(),
        body: beginCell().storeBool(false).storeUint(0b10101, 5).endCell().beginParse(),
    };
    expect(shortBody.loadOpcode()).toBeNull();

    val bouncedWithoutActualOpcode = ExtMessageRelaxedGeneric {
        info: externalInfo(),
        init: Maybe<Either<StateInit, Cell<StateInit>>>.none(),
        body: beginCell().storeBool(false).storeUint(0xFFFFFFFF, 32).endCell().beginParse(),
    };
    expect(bouncedWithoutActualOpcode.loadOpcode(true)).toBeNull();
}
