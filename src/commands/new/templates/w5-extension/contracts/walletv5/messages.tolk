struct (0x02) AddExtensionExtraAction {
    addr: address
}

struct (0x03) RemoveExtensionExtraAction {
    addr: address
}

struct (0x04) SetSignatureAllowedExtraAction {
    allowSignature: bool
}

type ExtraAction =
    | AddExtensionExtraAction
    | RemoveExtensionExtraAction
    | SetSignatureAllowedExtraAction

/// "OutActionList" is what's pushed to c5.
type OutActionsCell = cell

// "extra actions" are represented in a snake format: "data".ref("data.ref("data"...))
type SnakedExtraActions = RemainingBitsAndRefs

fun SnakedExtraActions.hasNext(self): bool {
    return self.remainingRefsCount() != 0;
}

fun SnakedExtraActions.getNext(self): SnakedExtraActions {
    return self.preloadRef().beginParse();
}

struct (0x6578746E) ExtensionActionRequest {
    queryId: uint64
    outActions: OutActionsCell?
    hasExtraActions: bool
    extraActions: SnakedExtraActions
}

struct (0x73696E74) InternalSignedRequest {
    walletId: uint32
    validUntil: uint32
    seqno: uint32
    outActions: OutActionsCell?
    hasExtraActions: bool
    extraActions: SnakedExtraActions
    // ends with a signature (512 bits), but it's parsed and compared separately
}

struct (0x7369676E) ExternalSignedRequest {
    walletId: uint32
    validUntil: uint32
    seqno: uint32
    outActions: OutActionsCell?
    hasExtraActions: bool
    extraActions: SnakedExtraActions
    // ends with a signature (512 bits), but it's parsed and compared separately
}
