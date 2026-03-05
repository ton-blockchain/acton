import "@local-acton/emulation/network"
import "@local-acton/ffi/ffi"
import "@local-acton/ffi/reverse-ffi"
import "@local-acton/testing/expect"

fun x2(x: int): int {
    return x * 2;
}

fun drop(): void asm "DROP";

fun combineConts(first: continuation, then: continuation): continuation asm "BOOLAND";

fun wrap<T, U>(f: T -> U): (T -> U) {
    val dropAsCont = fun(): int {
        drop();
    };
    return combineConts(dropAsCont, f);
}

get fun `test basic`() {
    expect(ffi.executeInRust(wrap(x2), 10)).toEqual(20);
}
