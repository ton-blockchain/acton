import "ffi"

fun ffi.testReverse(f: (int) -> (int), x: int, addr: address): int asm "501 EXTCALL";

