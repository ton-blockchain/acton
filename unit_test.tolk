import "lib/io"
import "lib/testing/expect"
import "lib/vm/vm"

fun Expectation<int>.toEqualWithFlags(self, right: int, param1: bool, param2: bool) {}

// get fun `test-unit`() {
//     expect(5 + 5).toEqualWithFlags(10, false, true);
//     expect(2 + 2).toEqual(1 + 3);
//
//     val freeze_limit = vm.getConfigParam<tuple>(14)
//         .get<slice>(3)
//         .skipBits(464)
//         .loadUint(64);
//     println(freeze_limit);
// }

fun main() {
    val freeze_limit = vm.getConfigParam<tuple>(14)
        .get<slice>(3)
        .skipBits(464)
        .loadUint(64);

    throw freeze_limit;
    // println(freeze_limit);
}
