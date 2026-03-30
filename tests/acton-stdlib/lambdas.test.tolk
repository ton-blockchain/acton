import "@local-acton/emulation/network"
import "@local-acton/ffi/ffi"
import "@local-acton/ffi/reverse-ffi"
import "@local-acton/testing/expect"

fun x2(x: int): int {
    return x * 2;
}

get fun `test basic`() {
    val res = ffi.__executeInSeparateLibemulator(x2, 300);
    expect(res).toEqual(600);
}
