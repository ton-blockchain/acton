struct ExtensionStorage {
    walletAddress: address
    admin: address
    subscriptionAmount: coins
    lastPaymentTime: uint32?
    paymentTimeInterval: uint32
}

fun ExtensionStorage.load(): ExtensionStorage {
    return ExtensionStorage.fromCell(contract.getData());
}

fun ExtensionStorage.save(self) {
    contract.setData(self.toCell());
}

// external
struct (0x7e8764ef) CollectPaymentFromWallet {
    queryId: uint64
}

struct (0x342364ac) WithdrawExtensionBalance {}

// internal
struct (0x283b4c3f) CancelSubscription {
    queryId: uint64
}

struct (0xc28364ef) ReceivePaymentFromWallet {}

struct (0x43c7641f) TopUp {}
