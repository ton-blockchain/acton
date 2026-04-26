enum Errors {
    NotOwner = 100
    CounterUnderflow = 0x1001
    InvalidMessage = 0xFFFF
}

struct Storage {
    id: uint32
    owner: address
    counter: uint32
}

fun Storage.load(): Storage {
    return Storage.fromCell(contract.getData());
}

fun Storage.save(self) {
    contract.setData(self.toCell());
}

struct (0x7e8764ef) IncreaseCounter {
    increaseBy: uint32
}

struct (0x283b4c3f) DecreaseCounter {
    decreaseBy: uint32
}

struct (0x3a752f06) ResetCounter {}
