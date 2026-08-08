fun contract.setLibraryCode(code: cell, mode: int): void
    asm "SETLIBCODE"

fun onInternalMessage(_: InMessage) {
    contract.setLibraryCode(contract.getData(), 2);
}

fun onBouncedMessage(_: InMessageBounced) {}
