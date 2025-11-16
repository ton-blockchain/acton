import "types"

type AllowedMessage = IncreaseCounter | ResetCounter

fun onInternalMessage(in: InMessage) {
    val msg = lazy AllowedMessage.fromSlice(in.body);

    match (msg) {
        IncreaseCounter => {
            var storage = lazy Storage.load();
            storage.counter += msg.increaseBy;
            storage.save();

            {
                val msg = createExternalLogMessage({
                    dest: createAddressNone(),
                    body: IncreaseCounter{
                        increaseBy: 100,
                    },
                });
                msg.send(SEND_MODE_REGULAR);
            }

            {
                val msg = createExternalLogMessage({
                    dest: any_address.fromCell(beginCell().storeUint(0b01, 2).storeUint(16, 9).storeUint(0xDEAD, 16).endCell()),
                    body: ResetCounter{},
                });
                msg.send(SEND_MODE_REGULAR);
            }
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
