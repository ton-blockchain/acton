import "types"

type AllowedMessage = IncreaseCounter | ResetCounter

fun onInternalMessage(in: InMessage) {
    val msg = lazy AllowedMessage.fromSlice(in.body);

    match (msg) {
        IncreaseCounter => {
            var storage = lazy Storage.load();
            storage.counter += msg.increaseBy;
            storage.save();
        }
        ResetCounter => {
            var storage = lazy Storage.load();
            storage.counter = 0;
            storage.save();
        }
        else => {
            assert (in.body.isEmpty()) throw 0xFFFF;
        }
    }
}

fun onBouncedMessage(in: InMessageBounced) {}

get fun currentCounter(): int {
    val storage = lazy Storage.load();
    return storage.counter;
}

get fun currentCounterDoubled(): (int, int) {
    val storage = lazy Storage.load();
    return (storage.counter, storage.counter * 2);
}

get fun currentCounterIfGraterThan100(): int? {
    val storage = lazy Storage.load();
    if (storage.counter > 100) {
        return storage.counter * 100;
    }
    return null;
}

get fun currentStorage(): Storage {
    val storage = lazy Storage.load();
    return storage;
}

get fun currentStorageOrNull(): Storage? {
    val storage = lazy Storage.load();
    if (storage.counter > 100) {
        return storage;
    }
    return null;
}

get fun double(x: int): int {
    val storage = lazy Storage.load();
    return storage.counter + x * 2;
}

get fun doubleBoth(x: int, y: int): int {
    val storage = lazy Storage.load();
    return storage.counter + x * 2 + y;
}

get fun doubleObject(p: Pair): Pair {
    val storage = lazy Storage.load();
    return Pair {
        x: p.x + storage.counter,
        y: p.y * 2,
    };
}
