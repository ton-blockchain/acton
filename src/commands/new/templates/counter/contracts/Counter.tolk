import "types"

contract Counter {
    storage: Storage
    incomingMessages: AllowedMessage
}

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
            assert (in.body.isEmpty()) throw Errors.InvalidMessage;
        }
    }
}

fun onBouncedMessage(_in: InMessageBounced) {}

get fun currentCounter(): int {
    val storage = lazy Storage.load();
    return storage.counter;
}
