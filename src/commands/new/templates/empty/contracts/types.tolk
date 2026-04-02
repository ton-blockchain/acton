enum Errors {
    NotOwner = 100
    InvalidMessage = 0xFFFF
}

struct Storage {
    owner: address
}

fun Storage.load(): Storage {
    return Storage.fromCell(contract.getData());
}

fun Storage.save(self) {
    contract.setData(self.toCell());
}

struct (0x2ce05111) ChangeOwner {
    newOwner: address
}
