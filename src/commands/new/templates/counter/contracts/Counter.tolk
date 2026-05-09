import "types"

contract Counter {
    author: "The Tolk Team"
    version: "1.0.0"
    description: "Simple counter contract with owner-controlled updates"
    storage: Storage
    incomingMessages: AllowedMessage
}

type AllowedMessage =
    | IncreaseCounter
    | DecreaseCounter
    | ResetCounter

fun onInternalMessage(in: InMessage) {
    val msg = lazy AllowedMessage.fromSlice(in.body);

    match (msg) {
        IncreaseCounter => {
            var storage = lazy Storage.load();
            assert (storage.owner == in.senderAddress) throw Errors.NotOwner;

            storage.counter += msg.increaseBy;
            storage.save();
        }
        DecreaseCounter => {
            var storage = lazy Storage.load();
            assert (storage.owner == in.senderAddress) throw Errors.NotOwner;
            assert (storage.counter >= msg.decreaseBy) throw Errors.CounterUnderflow;

            storage.counter -= msg.decreaseBy;
            storage.save();
        }
        ResetCounter => {
            var storage = lazy Storage.load();
            assert (storage.owner == in.senderAddress) throw Errors.NotOwner;

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

get fun owner(): address {
    val storage = lazy Storage.load();

    return storage.owner;
}
