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
    !ffi.assert_bin(
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
    !ffi.assert_bin(
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
    ffi.assert_fail(message, location);
    throw ASSERTION_FAILED;
}

/// Asserts that the function without arguments consumes less than the expected amount of gas.
/// 
/// Until Tolk supports anonymous functions, you need to pass the standalone function as a parameter.
/// 
/// Example:
/// ```
/// fun to_track_gas() {
///     // do something that consumes gas
/// }
/// 
/// get fun test_gas_consumption() {
///     Assert.consumesLessThan(to_track_gas, 100);
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
    ffi.assert_consumed_less(start, end, expected);
    return result;
}

/// Asserts that the function with single argument consumes less than the expected amount of gas.
/// 
/// Until Tolk supports anonymous functions, you need to pass the standalone function as a parameter.
/// 
/// Example:
/// ```
/// fun to_track_gas(a: int) {
///     // do something that consumes gas
/// }
/// 
/// get fun test_gas_consumption() {
///     Assert.consumesLessThan1(to_track_gas, 100, 100);
/// //                                         ^^^ this argument will be passed 
/// //                                             to the function as the first argument
/// }
/// ```
fun Assert.consumesLessThan1<Ret, Arg1>(fn: (Arg1) -> Ret, arg1: Arg1, expected: int) {
    val start = getGasConsumedAtTheMoment();
    val result = fn(arg1);
    val end = getGasConsumedAtTheMoment();
    ffi.assert_consumed_less(start, end, expected);
    return result;
}

/// Asserts that the function with two arguments consumes less than the expected amount of gas.
/// 
/// Until Tolk supports anonymous functions, you need to pass the standalone function as a parameter.
/// 
/// Example:
/// ```
/// fun to_track_gas(a: int, b: int) {
///     // do something that consumes gas
/// }
/// 
/// get fun test_gas_consumption() {
///     Assert.consumesLessThan2(to_track_gas, 100, 100, 100);
/// }
/// ```
fun Assert.consumesLessThan2<Ret, Arg1, Arg2>(fn: (Arg1, Arg2) -> Ret, arg1: Arg1, arg2: Arg2, expected: int) {
    val start = getGasConsumedAtTheMoment();
    val result = fn(arg1, arg2);
    val end = getGasConsumedAtTheMoment();
    ffi.assert_consumed_less(start, end, expected);
    return result;
}

/// Asserts that the function with three arguments consumes less than the expected amount of gas.
/// 
/// Until Tolk supports anonymous functions, you need to pass the standalone function as a parameter.
/// 
/// Example:
/// ```
/// fun to_track_gas(a: int, b: int, c: int) {
///     // do something that consumes gas
/// }
/// 
/// get fun test_gas_consumption() {
///     Assert.consumesLessThan3(to_track_gas, 100, 100, 100, 100);
/// }
/// ```
fun Assert.consumesLessThan3<Ret, Arg1, Arg2, Arg3>(fn: (Arg1, Arg2, Arg3) -> Ret, arg1: Arg1, arg2: Arg2, arg3: Arg3, expected: int) {
    val start = getGasConsumedAtTheMoment();
    val result = fn(arg1, arg2, arg3);
    val end = getGasConsumedAtTheMoment();
    ffi.assert_consumed_less(start, end, expected);
    return result;
}

/// Internal function to assert that the function consumed less than the expected amount of gas.
/// 
/// NOT FOR PUBLIC USE.
fun ffi.assert_consumed_less(start: int, end: int, expected: int) {
    val consumed = end - start - 26; // 26 gas cost of GASCONSUMED
    if (consumed > expected) {
        Assert.fail(format2("Gas consumption was expected to be less than or equal to {}, but {} was consumed", expected, consumed))
    }
}

/// Internal function to assert two values are equal or not equal.
///
/// NOT FOR PUBLIC USE.
fun ffi.assert_bin<V1, V2>(
    op: slice,
    left_name: slice,
    left: V1,
    right_name: slice,
    right: V2,
    message: slice = "",
    location: slice = "",
): bool
    asm "101 EXTCALL"

/// Internal function to fail the test with the given message and location.
///
/// NOT FOR PUBLIC USE.
fun ffi.assert_fail(message: slice = "", location: slice = ""): bool
    asm "100 EXTCALL"

fun ffi.type_name_by_opcode(id: int): slice?
    asm "14 EXTCALL"

