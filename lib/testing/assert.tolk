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
    !__assert_bin(
        "==",
        __typeName(left),
        left.__toTuple(),
        __typeName(right),
        right.__toTuple(),
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
    !__assert_bin(
        "!=",
        __typeName(left),
        left.__toTuple(),
        __typeName(right),
        right.__toTuple(),
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
    __assert_fail(message, location);
    throw ASSERTION_FAILED;
}

/// Internal function to assert two values are equal or not equal.
///
/// NOT FOR PUBLIC USE.
fun __assert_bin<V1, V2>(
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
fun __assert_fail(message: slice = "", location: slice = ""): bool
    asm "100 EXTCALL"
