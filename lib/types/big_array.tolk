/// Module defining `BigArray<T>`.
///
/// `BigArray<T>` is a chunked array implementation for cases when regular `array<T>`
/// capacity (`0..255`) is not enough.
///
/// Internally it stores items in bins, where each bin is `array<T>` with up to 255 items.
/// New bins are created lazily on demand.
/// Total capacity is up to `255 * 255 = 65025` items.
///
/// Example:
/// ```tolk
/// var list = BigArray<int>.createEmpty();
/// list.push(10);
/// list.push(20);
/// val first = list.first(); // 10
/// val last = list.last();   // 20
/// ```
struct BigArray<T> {
    private topLevel: array<array<T>> = [];
    private itemsCount: int = 0;
}

/// Creates an empty [BigArray].
///
/// Use this constructor to initialize a new chunked array.
fun BigArray<T>.createEmpty(): BigArray<T> {
    return BigArray<T> {
        topLevel: [],
        itemsCount: 0,
    }
}

/// Creates [BigArray] and fills it with all elements from [items].
///
/// Elements preserve the original order from [items].
/// All elements are copied into the first bin directly.
fun BigArray<T>.createFromArray(items: array<T>): BigArray<T> {
    if (items.size() == 0) {
        return BigArray<T>.createEmpty();
    }

    return BigArray<T> {
        topLevel: [items],
        itemsCount: items.size(),
    };
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
/// Fails if array capacity exceeds 65025 elements.
fun BigArray<T>.push(mutate self, value: T) {
    if (self.itemsCount >= 65025) {
        throw 5;
    }

    val binIdx = self.itemsCount / 255;
    while (self.topLevel.size() <= binIdx) {
        self.topLevel.push([]);
    }

    var bin = self.topLevel.get(binIdx);
    bin.push(value);
    self.topLevel.set(bin, binIdx);
    self.itemsCount += 1;
}

/// Removes the last element from [BigArray].
///
/// Fails if the array is empty.
fun BigArray<T>.pop(mutate self) {
    val binIdx = (self.itemsCount - 1) / 255;
    var bin = self.topLevel.get(binIdx);
    bin.pop();
    if (bin.size() == 0 && binIdx == self.topLevel.size() - 1) {
        self.topLevel.pop();
    } else {
        self.topLevel.set(bin, binIdx);
    }
    self.itemsCount -= 1;
}

/// Replaces element at [index] with [value].
///
/// Fails if [index] is out of bounds.
fun BigArray<T>.set(mutate self, value: T, index: int) {
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
