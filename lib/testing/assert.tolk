/// Module for assertion functions.
///
/// This module provides the foundational assertion logic used in tests.
/// While `expect` provides a high-level, fluent API, `Assert` offers direct static methods for verification.
///
/// Examples:
/// ```tolk
/// // Basic equality
/// Assert.equal(2 + 2, 4, "Math check");
///
/// // Gas usage check for a function
/// Assert.consumesLessThan(fun() {
///     someExpensiveOperation();
/// }, 10000);
/// ```

import "@stdlib/gas-payments"
import "@stdlib/reflection"
import "../ffi"
import "../fmt"

const ASSERTION_FAILED = 567

/// Root module for the assertion API.
///
/// This structure holds static methods for the assertion API.
///
/// Example:
/// ```tolk
/// Assert.equal(10, 10);
/// ```
///
/// For higher-level interface, use the `expect` function.
///
/// Example:
/// ```tolk
/// expect(10).toEqual(10);
/// ```
struct Assert

/// Asserts that two values are equal. If not, throws `ASSERTION_FAILED`.
///
/// Optional message can be provided to describe the assertion.
/// Optional location can be provided to specify the location of the assertion to
/// be printed in the error message.
///
/// This is a low-level function. Expectation API provides a higher-level interface
/// with automatic location tracking.
fun Assert.equal<V1, V2>(left: V1, right: V2, message: string = "", location: string = ""): void {
    ffi.assertComparison(
        "==",
        left,
        right,
        message,
        location,
    );
}

/// Asserts that two values are NOT equal. If they are, throws `ASSERTION_FAILED`.
///
/// Optional message can be provided to describe the assertion.
/// Optional location can be provided to specify the location of the assertion to
/// be printed in the error message.
///
/// This is a low-level function. Expectation API provides a higher-level interface
/// with automatic location tracking.
fun Assert.notEqual<V1, V2>(
    left: V1,
    right: V2,
    message: string = "",
    location: string = "",
): void {
    ffi.assertComparison(
        "!=",
        left,
        right,
        message,
        location,
    );
}

/// Fails the test with the given optional message.
///
/// Optional message can be provided to describe the failure.
/// Optional location can be provided to specify the location of the failure to
/// be printed in the error message.
fun Assert.fail(message: string, location: string = ""): never {
    ffi.assertFail(message, location);
    throw ASSERTION_FAILED;
}

/// Rejects the current fuzz input when condition is false.
///
/// Outside fuzz tests this is reported as a normal test failure.
fun Assert.assume(condition: bool, message: string = "", location: string = ""): void {
    if (!condition) {
        ffi.assumeReject(message, location);
        throw ASSERTION_FAILED;
    }
}

/// Asserts that the function consumes less than or equal to the expected amount of gas.
/// Returns the callback result.
///
/// Gas is compared using `end - start - 26`.
/// The fixed `26` gas measurement overhead is subtracted before the `<= expected` check.
///
/// Example:
/// ```tolk
/// get fun `test gas consumption`() {
///     val result = Assert.consumesLessThan(fun() {
///         // do something that consumes gas
///         // (capturing outer variables is allowed)
///         return 42;
///     }, 100);
///     Assert.equal(result, 42);
/// }
/// ```
fun Assert.consumesLessThan<Ret>(fn: () -> Ret, expected: int): Ret {
    val start = getGasConsumedAtTheMoment();
    val result = fn();
    val end = getGasConsumedAtTheMoment();

    val consumed = end - start - 26; // 26 gas cost of GASCONSUMED
    if (consumed > expected) {
        Assert.fail(
            format(
                "Gas consumption was expected to be less than or equal to {}, but {} was consumed",
                expected,
                consumed,
            ),
        );
    }

    return result;
}

/// Asserts that two values are equal as decimals with given precision.
///
/// Optional message can be provided to describe the assertion.
/// Optional location can be provided to specify the location of the assertion to
/// be printed in the error message.
///
/// This is a low-level function. Expectation API provides a higher-level interface
/// with automatic location tracking.
fun Assert.equalDecimal<T>(
    left: T,
    right: T,
    decimals: int,
    message: string = "",
    location: string = "",
): void {
    assert (ffi.assertDecimal(location, message, decimals, right, left)) throw ASSERTION_FAILED;
}

fun ffi.assertBin<V1, V2>(
    op: string,
    leftTyIdx: int,
    left: V1,
    rightTyIdx: int,
    right: V2,
    message: string,
    location: string,
): bool
    asm "101 EXTCALL"

fun ffi.assertComparison<V1, V2>(
    op: string,
    left: V1,
    right: V2,
    message: string,
    location: string,
): void {
    assert (
        ffi.assertBin(
            op,
            reflect.typeUniqueIdxOfObject(left),
            left.toTuple(),
            reflect.typeUniqueIdxOfObject(right),
            right.toTuple(),
            message,
            location,
        )
    ) throw ASSERTION_FAILED;
}

fun ffi.assertFail(message: string, location: string): bool
    asm "100 EXTCALL"

fun ffi.assertDecimal<T>(location: string, message: string, decimals: int, right: T, left: T): bool
    asm "106 EXTCALL"

fun ffi.assumeReject(message: string, location: string): bool
    asm "107 EXTCALL"

fun ffi.typeNameByOpcode(id: int): string?
    asm "14 EXTCALL"
