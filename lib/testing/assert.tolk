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
/// // Gas usage check for a void function
/// Assert.consumesLessThan(fun() {
///     someExpensiveOperation();
/// }, 10000);
///
/// // Gas usage check for a function with arguments
/// Assert.consumesLessThan2(fun(a: int, b: int) {
///     return a * b;
/// }, 10, 20, 500);
/// ```

import "@stdlib/gas-payments"
import "../fmt"
import "../ffi/ffi"

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
fun Assert.equal<V1, V2>(left: V1, right: V2, message: slice = "", location: slice = "") {
    if (
    !ffi.assertBin(
        "==",
        left.typeNameOfObject(),
        left.toTuple(),
        right.typeNameOfObject(),
        right.toTuple(),
        message,
        location
    )
    ) {
        throw ASSERTION_FAILED;
    }
}

/// Asserts that two values are NOT equal. If they are, throws `ASSERTION_FAILED`.
///
/// Optional message can be provided to describe the assertion.
/// Optional location can be provided to specify the location of the assertion to
/// be printed in the error message.
///
/// This is a low-level function. Expectation API provides a higher-level interface
/// with automatic location tracking.
fun Assert.notEqual<V1, V2>(left: V1, right: V2, message: slice = "", location: slice = "") {
    if (
    !ffi.assertBin(
        "!=",
        left.typeNameOfObject(),
        left.toTuple(),
        right.typeNameOfObject(),
        right.toTuple(),
        message,
        location
    )
    ) {
        throw ASSERTION_FAILED;
    }
}

/// Fails the test with the given optional message.
///
/// Optional message can be provided to describe the failure.
/// Optional location can be provided to specify the location of the failure to
/// be printed in the error message.
fun Assert.fail(message: slice = "", location: slice = ""): never {
    ffi.assertFail(message, location);
    throw ASSERTION_FAILED;
}

/// Asserts that the function without arguments consumes less than the expected amount of gas.
///
/// Example:
/// ```tolk
/// get fun test_gas_consumption() {
///     Assert.consumesLessThan(fun do_something() {
///         // do something that consumes gas
///     }, 100);
/// }
/// ```
///
/// If you need to pass arguments to the function, you can use the `consumesLessThan1`,
/// `consumesLessThan2` or `consumesLessThan3` functions which are similar to the `consumesLessThan`
/// but accept arguments.
fun Assert.consumesLessThan<Ret>(fn: () -> Ret, expected: int) {
    val start = getGasConsumedAtTheMoment();
    val result = fn();
    val end = getGasConsumedAtTheMoment();
    ffi.assertConsumedLess(start, end, expected);
    return result;
}

/// Asserts that the function with single argument consumes less than the expected amount of gas.
///
/// Example:
/// ```tolk
/// get fun test_gas_consumption() {
///     Assert.consumesLessThan1(fun do_something(a: int, b: int) {
///         // do something that consumes gas
///     }, 100, 100);
/// //     ^^^  ^^^ this argument will be passed to the function as the second argument
/// //     |
/// //     this argument will be passed to the function as the first argument
/// }
/// ```
fun Assert.consumesLessThan1<Ret, Arg1>(fn: (Arg1) -> Ret, arg1: Arg1, expected: int) {
    val start = getGasConsumedAtTheMoment();
    val result = fn(arg1);
    val end = getGasConsumedAtTheMoment();
    ffi.assertConsumedLess(start, end, expected);
    return result;
}

/// Asserts that the function with two arguments consumes less than the expected amount of gas.
///
/// Example:
/// ```tolk
/// get fun test_gas_consumption() {
///     Assert.consumesLessThan2(fun do_something(a: int, b: int) {
///         // do something that consumes gas
///     }, 100, 100, 100);
/// //     ^^^  ^^^  ^^^ this argument will be passed to the function as the third argument
/// //     |    |
/// //     |    this argument will be passed to the function as the second argument
/// //     |
/// //     this argument will be passed to the function as the first argument
/// }
/// ```
fun Assert.consumesLessThan2<Ret, Arg1, Arg2>(
    fn: (Arg1, Arg2) -> Ret,
    arg1: Arg1,
    arg2: Arg2,
    expected: int
) {
    val start = getGasConsumedAtTheMoment();
    val result = fn(arg1, arg2);
    val end = getGasConsumedAtTheMoment();
    ffi.assertConsumedLess(start, end, expected);
    return result;
}

/// Asserts that the function with three arguments consumes less than the expected amount of gas.
///
/// Example:
/// ```tolk
/// get fun test_gas_consumption() {
///     Assert.consumesLessThan3(fun do_something(a: int, b: int, c: int) {
///         // do something that consumes gas
///     }, 100, 100, 100, 100);
/// //     ^^^  ^^^  ^^^  ^^^ this argument will be passed to the function as the fourth argument
/// //     |    |    |
/// //     |    |    this argument will be passed to the function as the third argument
/// //     |    |
/// //     |    this argument will be passed to the function as the second argument
/// //     |
/// //     this argument will be passed to the function as the first argument
/// }
/// ```
fun Assert.consumesLessThan3<Ret, Arg1, Arg2, Arg3>(
    fn: (Arg1, Arg2, Arg3) -> Ret,
    arg1: Arg1,
    arg2: Arg2,
    arg3: Arg3,
    expected: int
) {
    val start = getGasConsumedAtTheMoment();
    val result = fn(arg1, arg2, arg3);
    val end = getGasConsumedAtTheMoment();
    ffi.assertConsumedLess(start, end, expected);
    return result;
}

/// Fails the test with wallet not found error.
///
/// This function is used internally by the framework when a wallet is not found.
/// It provides detailed information about available wallets and instructions on how to add them.
///
/// NOT FOR PUBLIC USE.
fun Assert.failWalletNotFound(walletName: slice, location: slice = ""): never {
    ffi.failWalletNotFound(walletName, location);
    throw ASSERTION_FAILED;
}

/// Asserts that two values are equal as decimals with given precision.
///
/// Optional message can be provided to describe the assertion.
/// Optional location can be provided to specify the location of the assertion to
/// be printed in the error message.
///
/// This is a low-level function. Expectation API provides a higher-level interface
/// with automatic location tracking.
fun Assert.equalDecimal<T>(left: T, right: T, decimals: int, message: slice = "", location: slice = "") {
    if (
        !ffi.assertDecimal(
            location,
            message,
            decimals,
            right,
            left
        )
    ) {
        throw ASSERTION_FAILED;
    }
}

/// Internal function to assert that the function consumed less than the expected amount of gas.
///
/// NOT FOR PUBLIC USE.
fun ffi.assertConsumedLess(start: int, end: int, expected: int) {
    val consumed = end - start - 26; // 26 gas cost of GASCONSUMED
    if (consumed > expected) {
        Assert.fail(format2("Gas consumption was expected to be less than or equal to {}, but {} was consumed", expected, consumed))
    }
}

/// Internal function to assert two values are equal or not equal.
///
/// NOT FOR PUBLIC USE.
fun ffi.assertBin<V1, V2>(
    op: slice,
    leftName: slice,
    left: V1,
    rightName: slice,
    right: V2,
    message: slice = "",
    location: slice = "",
): bool
    asm "101 EXTCALL"

/// Internal function to fail the test with the given message and location.
///
/// NOT FOR PUBLIC USE.
fun ffi.assertFail(message: slice = "", location: slice = ""): bool
    asm "100 EXTCALL"

/// Internal function to fail when wallet is not found.
///
/// NOT FOR PUBLIC USE.
fun ffi.failWalletNotFound(walletName: slice, location: slice = ""): bool
    asm "105 EXTCALL"

fun ffi.assertDecimal<T>(location: slice, message: slice, decimals: int, right: T, left: T): bool
    asm "106 EXTCALL"

fun ffi.typeNameByOpcode(id: int): slice?
    asm "14 EXTCALL"

