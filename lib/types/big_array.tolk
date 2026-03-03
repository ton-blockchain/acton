/// Module defining `BigArray<T>`.
///
/// `BigArray<T>` is a chunked array implementation for cases when regular `array<T>`
/// capacity (`0..255`) is not enough.
///
/// Internally it stores items in 255 bins, each bin is `array<T>` with up to 255 items,
/// so total capacity is up to `255 * 255 = 65025` items.
///
/// Example:
/// ```tolk
/// var list = BigArray<int>.fromTuple([false, [] as array<array<int>>, 0]);
/// list.init();
/// list.push(10);
/// list.push(20);
/// val first = list.first(); // 10
/// val last = list.last();   // 20
/// ```
struct BigArray<T> {
    private isInit: bool = false;
    private topLevel: array<array<T>>;
    private itemsCount: int = 0;
}

/// Initializes internal storage of [BigArray].
///
/// This method allocates 255 empty bins in [BigArray.topLevel] once.
/// Subsequent calls are no-op.
///
/// Most mutating methods (`push`, `pop`, `set`) call `init()` automatically.
fun BigArray<T>.init(mutate self) {
    if (!self.isInit) {
        repeat (255) {
            self.topLevel.push([]);
        }
        self.isInit = true;
    }
}

/// Returns the first element of a non-empty [BigArray].
///
/// Fails if the array is empty.
fun BigArray<T>.first(self): T {
    return self.topLevel.get(0).get(0);
}

/// Returns current number of elements in [BigArray].
fun BigArray<T>.size(self): int {
    return self.itemsCount;
}

/// Returns the last element of a non-empty [BigArray].
///
/// Fails if the array is empty.
fun BigArray<T>.last(self): T {
    val binIdx = (self.itemsCount - 1) / 255;
    val bin = self.topLevel.get(binIdx);
    return bin.last();
}

/// Appends [value] to the end of [BigArray].
///
/// Initializes storage lazily on first call.
/// Fails if array capacity exceeds 65025 elements.
fun BigArray<T>.push(mutate self, value: T) {
    self.init();

    val binIdx = self.itemsCount / 255;
    var bin = self.topLevel.get(binIdx);
    bin.push(value);
    self.topLevel.set(bin, binIdx);
    self.itemsCount += 1;
}

/// Removes the last element from [BigArray].
///
/// Initializes storage lazily on first call.
/// Fails if the array is empty.
fun BigArray<T>.pop(mutate self) {
    self.init();

    val binIdx = (self.itemsCount - 1) / 255;
    var bin = self.topLevel.get(binIdx);
    bin.pop();
    self.topLevel.set(bin, binIdx);
    self.itemsCount -= 1;
}

/// Replaces element at [index] with [value].
///
/// Initializes storage lazily on first call.
/// Fails if [index] is out of bounds.
fun BigArray<T>.set(mutate self, value: T, index: int) {
    self.init();

    val binIdx = index / 255;
    var bin = self.topLevel.get(binIdx);
    val idxInBin = index % 255;
    bin.set(value, idxInBin);
    self.topLevel.set(bin, binIdx);
}

/// Returns element at [index].
///
/// Fails if [index] is out of bounds.
fun BigArray<T>.get(self, index: int): T {
    val binIdx = index / 255;
    var bin = self.topLevel.get(binIdx);
    val idxInBin = index % 255;
    return bin.get(idxInBin);
}
