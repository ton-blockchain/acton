struct WalletStorage {
    jettonBalance: coins
    ownerAddress: address
    minterAddress: address
}

struct MinterStorage {
    totalSupply: coins
    adminAddress: address
    content: cell
    jettonWalletCode: cell
}


fun MinterStorage.load(): MinterStorage {
    return MinterStorage.fromCell(contract.getData())
}

fun MinterStorage.save(self) {
    contract.setData(self.toCell())
}


fun WalletStorage.load(): WalletStorage {
    return WalletStorage.fromCell(contract.getData())
}

fun WalletStorage.save(self) {
    contract.setData(self.toCell())
}
