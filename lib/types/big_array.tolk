struct BigArray<T> {
    private isInit: bool = false;
    private topLevel: array<array<T>>;
    private itemsCount: int = 0;
}

fun BigArray<T>.init(mutate self) {
    if (!self.isInit) {
        repeat (255) {
            self.topLevel.push([]);
        }
        self.isInit = true;
    }
}

fun BigArray<T>.first(self): T {
    return self.topLevel.get(0).get(0);
}

fun BigArray<T>.size(self): int {
    return self.itemsCount;
}

fun BigArray<T>.last(self): T {
    val binIdx = (self.itemsCount - 1) / 255;
    val bin = self.topLevel.get(binIdx);
    return bin.last();
}

fun BigArray<T>.push(mutate self, value: T) {
    self.init();

    val binIdx = self.itemsCount / 255;
    var bin = self.topLevel.get(binIdx);
    bin.push(value);
    self.topLevel.set(bin, binIdx);
    self.itemsCount += 1;
}

fun BigArray<T>.pop(mutate self) {
    self.init();

    val binIdx = (self.itemsCount - 1) / 255;
    var bin = self.topLevel.get(binIdx);
    bin.pop();
    self.topLevel.set(bin, binIdx);
    self.itemsCount -= 1;
}

fun BigArray<T>.set(mutate self, value: T, index: int) {
    self.init();

    val binIdx = index / 255;
    var bin = self.topLevel.get(binIdx);
    val idxInBin = index % 255;
    bin.set(value, idxInBin);
    self.topLevel.set(bin, binIdx);
}

fun BigArray<T>.get(self, index: int): T {
    val binIdx = index / 255;
    var bin = self.topLevel.get(binIdx);
    val idxInBin = index % 255;
    return bin.get(idxInBin);
}

