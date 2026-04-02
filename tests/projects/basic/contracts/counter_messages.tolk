struct Storage {
    id: uint32
    counter: uint32
}

fun Storage.load(): Storage {
    return Storage.fromCell(contract.getData());
}

fun Storage.save(self) {
    contract.setData(self.toCell());
}

struct (0x7e8764ef) IncreaseCounter {
    queryId: uint64
    increaseBy: uint32
}

struct (0x3a752f06) ResetCounter {
    queryId: int32
}
