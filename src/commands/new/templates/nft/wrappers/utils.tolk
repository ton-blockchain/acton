import "@acton/emulation/network"
import "@contracts/messages"
import "NftItem.gen"

/// Sends `AskToChangeOwnership` with `forwardPayload` inlined into the message body.
fun NftItem.sendTransferPayloadInline(
    self,
    from: address,
    queryId: uint64,
    newOwnerAddress: address,
    sendExcessesTo: address?,
    customPayload: cell?,
    forwardTonAmount: coins,
    forwardPayload: slice,
    config: SendParams = {},
): SendResultList {
    return self.sendAskToChangeOwnership(
        from,
        queryId,
        newOwnerAddress,
        sendExcessesTo,
        customPayload,
        forwardTonAmount,
        PayloadInline { value: forwardPayload },
        config,
    );
}

/// Sends `AskToChangeOwnership` with `forwardPayload` stored in a separate cell ref.
fun NftItem.sendTransferPayloadInRef(
    self,
    from: address,
    queryId: uint64,
    newOwnerAddress: address,
    sendExcessesTo: address?,
    customPayload: cell?,
    forwardTonAmount: coins,
    forwardPayload: cell,
    config: SendParams = {},
): SendResultList {
    return self.sendAskToChangeOwnership(
        from,
        queryId,
        newOwnerAddress,
        sendExcessesTo,
        customPayload,
        forwardTonAmount,
        PayloadInRef { value: forwardPayload as Cell<RemainingBitsAndRefs> },
        config,
    );
}

/// Sends `AskToChangeOwnership` picking inline vs ref storage for `forwardPayload`
/// based on how much fits alongside the rest of the body.
fun NftItem.sendTransferPayloadAuto(
    self,
    from: address,
    queryId: uint64,
    newOwnerAddress: address,
    sendExcessesTo: address?,
    customPayload: cell?,
    forwardTonAmount: coins,
    forwardPayload: slice,
    config: SendParams = {},
): SendResultList {
    val (bodyBits, bodyRefs) = AskToChangeOwnership {
        queryId,
        newOwnerAddress,
        sendExcessesTo,
        customPayload,
        forwardTonAmount,
        forwardPayload: PayloadInline { value: createEmptySlice() },
    }.toCell()
        .beginParse()
        .remainingBitsAndRefsCount();

    val fitsInline = (1023 - bodyBits) > forwardPayload.remainingBitsCount() &&
    (4 - bodyRefs) > forwardPayload.remainingRefsCount();

    return fitsInline
        ? self.sendTransferPayloadInline(
            from,
            queryId,
            newOwnerAddress,
            sendExcessesTo,
            customPayload,
            forwardTonAmount,
            forwardPayload,
            config,
        )
        : self.sendTransferPayloadInRef(
            from,
            queryId,
            newOwnerAddress,
            sendExcessesTo,
            customPayload,
            forwardTonAmount,
            forwardPayload.toCell(),
            config,
        );
}
