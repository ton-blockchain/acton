struct (0x71000001) Trigger {
    queryId: uint64
}

struct (0x71000002) PaymentSent {
    queryId: uint64
    amount: coins
    recipient: address
}

fun onInternalMessage(in: InMessage) {
    if (in.body.isEmpty()) {
        return;
    }

    val request = Trigger.fromSlice(in.body);
    createExternalLogMessage({
        dest: createAddressNone(),
        body: PaymentSent {
            queryId: request.queryId,
            amount: ton("1.25"),
            recipient: in.senderAddress,
        },
    }).send(SEND_MODE_REGULAR);

    // Keep an internal message between the logs so their dictionary keys are not consecutive.
    createMessage({
        bounce: false,
        value: ton("0.05"),
        dest: contract.getAddress(),
        body: createEmptyCell(),
    }).send(SEND_MODE_REGULAR);

    val destination = beginCell()
        .storeUint(0b01, 2)
        .storeUint(16, 9)
        .storeUint(0xBEEF, 16)
        .endCell()
        .beginParse()
        .loadAddressAny();
    createExternalLogMessage({
        dest: destination,
        // A prefixless cell payload must not display its type as an opcode name.
        body: beginCell()
            .storeRef(
                beginCell().storeUint(42, 32).storeRef("ext-out inspector demo".toCell()).endCell(),
            )
            .endCell(),
    }).send(SEND_MODE_REGULAR);
}

fun onBouncedMessage(_: InMessageBounced) {}
