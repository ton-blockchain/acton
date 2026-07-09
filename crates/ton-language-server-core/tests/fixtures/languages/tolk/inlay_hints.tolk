tolk 1.0

const SIMPLE = 1
const COMPUTED = 1 + 2
const REFERENCED = COMPUTED * 4
const BOOLEAN = REFERENCED == 12

enum Status {
    None,
    Ready,
    Explicit = 7,
    Next,
    Hex = 0x10,
    Truthy = true,
    AfterTruthy,
}

struct Payload {
    value: int
}

fun deliver(destination: int, amount: int) {}

fun Payload.add(self, amount: int): int {
    return self.value + amount;
}

get fun currentCounter(): int {
    return 0;
}

@method_id(123)
get fun explicitId(): int {
    return 0;
}

get fun `test skipped`(): int {
    return 0;
}

fun main() {
    val payload = Payload { value: 1 };
    deliver(100, 25);
    payload.add(2);
}
