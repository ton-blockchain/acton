import "@local-acton/emulation/network"
import "@local-acton/ffi/ffi"
import "@local-acton/ffi/reverse-ffi"
import "@local-acton/testing/expect"

fun x2(x: int): int {
    return x * 2;
}

fun drop(): void asm "DROP";

fun wrap(f: int -> int): int {
    val combined = fun(fx: int -> int): int {
        drop();
        fx(10);
    };
    val res = ffi.testReverse(combined, f, net.randomAddress());
    return res;
}

get fun `test basic`() {
    expect(wrap(x2)).toEqual(20);
}
