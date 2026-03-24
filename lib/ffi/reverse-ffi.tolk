import "../fmt"
import "ffi"
import "@stdlib/reflection"
import "../testing/assert"

// not for public use. This functions calls arbitrary function in acton runtime and returns the result
// this function in used for internal acton tests
fun ffi.__executeInRust(functionToCall: continuation, args: array<unknown>, on: address): array<unknown>? asm "501 EXTCALL";

fun ffi.executeInRust<Ret, Args = array<unknown>>(functionToCall: (Args) -> Ret, args: Args? = null): Ret {
    val res = ffi.__executeInRust(functionToCall as unknown as continuation, args.toTuple(), contract.getAddress());
    if(res == null) {
        Assert.fail("__executeInRust unexpectidly returned null");
    }
    else {
        try {
            return Ret.fromTuple(res);
        } catch (e) {
            if (e == 7) {
                Assert.fail(format2("Calling for cont returned '{}' that cannot be deserialized to '{}'. Make sure you passed all parameters to the get method.",res, reflect.typeNameOf<Ret>()))
            }
            throw e
        }
    }
}
