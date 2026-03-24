import "@local-acton/emulation/network"
import "@local-acton/ffi/ffi"
import "@local-acton/ffi/reverse-ffi"
import "@local-acton/testing/expect"
import "@local-acton/io"

fun x2(x: int): int {
    return x * 2;
}
fun combineConts(first: continuation, then: continuation): continuation asm """
    BOOLOR
"""

fun pushint5(): void asm "5 PUSHINT";

fun nop(): void asm "NOP";
fun dropRetalt(): void asm "DROP RETALT";

fun withDrop<T, U>(f: T -> U): (T -> U) {
    val dropAsCont = fun(): void {
        dropRetalt();
    };
    return combineConts(dropAsCont as unknown as continuation, f as unknown as continuation) as unknown as (T -> U);
}

get fun `test basic`() {
    val test = withDrop(x2);
    val res = ffi.executeInRust(test, 300);
    println(res);
}
