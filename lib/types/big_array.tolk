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
    private arr: [array<array<T>>, int] = [[], 0]
}

/// Creates an empty [BigArray].
///
/// Use this constructor to initialize a new chunked array.
fun BigArray<T>.createEmpty(): BigArray<T> {
    return BigArray<T> {
        arr: [[], 0],
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
        arr: [[items], items.size()],
    };
}

/// Returns the first element of a non-empty [BigArray].
///
/// Fails if the array is empty.
fun BigArray<T>.first(self): T {
    val [topLevel, _] = self.arr;
    return topLevel.get(0).get(0);
}

/// Returns current number of elements in [BigArray].
fun BigArray<T>.size(self): int {
    val [_, itemsCount] = self.arr;
    return itemsCount;
}

/// Returns the last element of a non-empty [BigArray].
///
/// Fails if the array is empty.
fun BigArray<T>.last(self): T {
    val [topLevel, itemsCount] = self.arr;
    val binIdx = (itemsCount - 1) / 255;
    val bin = topLevel.get(binIdx);
    return bin.last();
}

/// Appends [value] to the end of [BigArray].
///
/// Fails if array capacity exceeds 65025 elements.
fun BigArray<T>.push(mutate self, value: T) {
    var [topLevel, itemsCount] = self.arr;

    if (itemsCount >= 65025) {
        throw 5;
    }

    val binIdx = itemsCount / 255;
    while (topLevel.size() <= binIdx) {
        topLevel.push([]);
    }

    var bin = topLevel.get(binIdx);
    bin.push(value);
    topLevel.set(bin, binIdx);
    itemsCount += 1;

    self.arr = [topLevel, itemsCount];
}

/// Removes the last element from [BigArray].
///
/// Fails if the array is empty.
fun BigArray<T>.pop(mutate self) {
    var [topLevel, itemsCount] = self.arr;
    val binIdx = (itemsCount - 1) / 255;
    var bin = topLevel.get(binIdx);
    bin.pop();
    if (bin.size() == 0 && binIdx == topLevel.size() - 1) {
        topLevel.pop();
    } else {
        topLevel.set(bin, binIdx);
    }
    itemsCount -= 1;

    self.arr = [topLevel, itemsCount];
}

/// Replaces element at [index] with [value].
///
/// Fails if [index] is out of bounds.
fun BigArray<T>.set(mutate self, value: T, index: int) {
    var [topLevel, itemsCount] = self.arr;
    val binIdx = index / 255;
    var bin = topLevel.get(binIdx);
    val idxInBin = index % 255;
    bin.set(value, idxInBin);
    topLevel.set(bin, binIdx);

    self.arr = [topLevel, itemsCount];
}

/// Returns element at [index].
///
/// Fails if [index] is out of bounds.
fun BigArray<T>.get(self, index: int): T {
    val [topLevel, _] = self.arr;
    val binIdx = index / 255;
    val bin = topLevel.get(binIdx);
    val idxInBin = index % 255;
    return bin.get(idxInBin);
}
