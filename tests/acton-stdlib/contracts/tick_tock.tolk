import "tick_tock_types"

fun onRunTickTock(isTock: bool) {
    var storage = lazy TickTockStorage.load();
    if (isTock) {
        storage.tockCount += 1;
    } else {
        storage.tickCount += 1;
    }
    storage.save();
}

fun onInternalMessage(_: InMessage) {}

fun onBouncedMessage(_: InMessageBounced) {}

get fun tickCount(): int {
    val storage = lazy TickTockStorage.load();
    return storage.tickCount;
}

get fun tockCount(): int {
    val storage = lazy TickTockStorage.load();
    return storage.tockCount;
}
