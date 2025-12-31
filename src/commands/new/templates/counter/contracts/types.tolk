struct Storage {
    id: uint32
    counter: uint32
}

fun Storage.load() {
    return Storage.fromCell(contract.getData());
}

fun Storage.save(self) {
    contract.setData(self.toCell());
}

struct (0x7e8764ef) IncreaseCounter {
    increaseBy: uint32
}

struct (0x3a752f06) ResetCounter {}
