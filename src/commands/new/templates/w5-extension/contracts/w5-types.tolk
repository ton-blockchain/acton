struct (0x03) W5RemoveExtensionExtraAction {
    addr: address
}

struct (0x6578746E) W5ExtensionActionRequest {
    queryId: uint64
    outActions: cell?
    hasExtraActions: bool
    extraActions: RemainingBitsAndRefs
}

const W5_ACTION_SEND_MSG_PREFIX = 0x0ec3c86d

// there is no not-manual pretty way to serialize this
fun createSingleWalletSendAction(messageCell: cell): cell {
    return beginCell()
        .storeRef(createEmptyCell())
        .storeUint(W5_ACTION_SEND_MSG_PREFIX, 32)
        .storeUint(SEND_MODE_PAY_FEES_SEPARATELY | SEND_MODE_IGNORE_ERRORS, 8)
        .storeRef(messageCell)
        .endCell();
}
