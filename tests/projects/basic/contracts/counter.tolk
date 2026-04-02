import "counter_messages"

type AllowedMessage = IncreaseCounter | ResetCounter

fun onInternalMessage(in: InMessage) {
    val msg = lazy AllowedMessage.fromSlice(in.body);
    // SLOT_2: debug.printString("Hello World");

    match (msg) {
        IncreaseCounter => {
            var storage = lazy Storage.load();
            storage.counter += msg.increaseBy;
            storage.save();
            // SLOT_1: throw 10;
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

fun onBouncedMessage(_: InMessageBounced) {}

get fun currentCounter(): int {
    val storage = lazy Storage.load();
    return storage.counter;
}

get fun initialId(): int {
    val storage = lazy Storage.load();
    return storage.id;
}
