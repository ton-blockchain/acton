import "types"

contract Counter {
    storage: Storage
    incomingMessages: AllowedMessage
}

type AllowedMessage = IncreaseCounter | DecreaseCounter | ResetCounter

fun onInternalMessage(in: InMessage) {
    val msg = lazy AllowedMessage.fromSlice(in.body);

    match (msg) {
        IncreaseCounter => {
            var storage = lazy Storage.load();

            storage.counter += msg.increaseBy;
            storage.save();
        }
        DecreaseCounter => {
            var storage = lazy Storage.load();

            assert (storage.counter >= msg.decreaseBy) throw 0x1001;
            storage.counter -= msg.decreaseBy;
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

fun onBouncedMessage(_in: InMessageBounced) {}

get fun currentCounter(): int {
    val storage = lazy Storage.load();

    return storage.counter;
}
