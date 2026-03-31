import "messages"

type AllowedMessage = Msg | Msg2

fun onInternalMessage(in: InMessage) {
    val msg = lazy AllowedMessage.fromSlice(in.body2);

    match (msg) {
        Msg => {}
        Msg2 => {}
    }
}

fun onBouncedMessage(_: InMessageBounced) {}
