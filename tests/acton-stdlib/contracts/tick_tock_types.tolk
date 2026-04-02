struct TickTockStorage {
    tickCount: uint32
    tockCount: uint32
}

fun TickTockStorage.load(): TickTockStorage {
    return TickTockStorage.fromCell(contract.getData());
}

fun TickTockStorage.save(self) {
    contract.setData(self.toCell());
}
