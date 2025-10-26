import "types"

type AllowedMessage = IncreaseCounter | ResetCounter

@inline_ref
fun processMessage(body: slice) {
    val msg = lazy AllowedMessage.fromSlice(body);

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
            assert (body.isEmpty()) throw 0xFFFF;
        }
    }
}

fun onInternalMessage(in: InMessage) {
    processMessage(in.body);
}

fun onBouncedMessage(in: InMessageBounced) {}

get fun currentCounter(): int {
    return foo();
}

@inline_ref
fun foo() {
    val storage = lazy Storage.load();
    return storage.counter;
}
