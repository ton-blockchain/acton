import "@stdlib/reflection"
import "ffi"
import "../fmt"
import "../testing/assert"

// not for public use. This functions calls arbitrary function in acton runtime and returns the result
// this function in used for internal acton tests
fun ffi.__executeInSeparateLibemulatorInternal(
    functionToCall: continuation,
    args: array<unknown>,
    on: address,
): array<unknown>?
    asm "501 EXTCALL"

// This function is needed only for testing Acton functionality. You probably won't need that in any time
fun ffi.__executeInSeparateLibemulator<Ret, Args = array<unknown>>(
    functionToCall: (Args) -> Ret,
    args: Args? = null,
): Ret {
    val res = ffi.__executeInSeparateLibemulatorInternal(
        functionToCall as unknown as continuation,
        args.toTuple(),
        contract.getAddress(),
    );
    if (res == null) {
        Assert.fail("__executeInRust unexpectedly returned null");
    } else {
        try {
            return Ret.fromTuple(res);
        } catch (e) {
            if (e == 7) {
                Assert.fail(
                    format2(
                        "Calling for cont returned '{}' that cannot be deserialized to '{}'.",
                        res,
                        reflect.typeNameOf<Ret>(),
                    ),
                );
            }
            throw e;
        }
    }
}
