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

/// Internal struct to represent an expectation for a value.
/// 
/// Don't use this struct directly. Use the `expect` function instead.
/// 
/// Example:
/// ```tolk
/// expect(10).toEqual(10);
/// ```
struct Expectation<T> {
    /// The value to be expected.
    private value: T
    /// The type of the value to be expected.
    private value_type: slice
    /// The location of the expectation to be printed in the error message.
    /// This field is set by the test framework and should not be set manually.
    private location: slice = ""
}

/// Internal function to create a new expectation for the given value.
/// 
/// Don't use this function directly. Use the `expect` function instead.
/// 
/// Example:
/// ```tolk
/// expect(10).toEqual(10);
/// ```
fun Expectation<T>.create(value: T, value_type: slice, location: slice = "") {
    return Expectation<T> {
        value,
        value_type,
        location,
    };
}

/// Asserts that actual value is equal to expected value.
/// 
/// Example:
/// ```tolk
/// expect(10).toEqual(10);
/// ```
fun Expectation<T>.toEqual<U>(self, expected: U) {
    Assert.equal(self.value, expected, "expect(actual).toEqual(expected)", self.location);
}

/// Asserts that actual value is not equal to expected value.
/// 
/// Example:
/// ```tolk
/// expect(10).toNotEqual(20);
/// ```
fun Expectation<T>.toNotEqual<U>(self, expected: U) {
    Assert.notEqual(self.value, expected, "expect(actual).toNotEqual(expected)", self.location);
}

/// Asserts that actual value is null.
/// 
/// Example:
/// ```tolk
/// expect(null).toBeNull();
/// ```
fun Expectation<T?>.toBeNull(self) {
    Assert.equal(self.value, null, "expect(actual).toBeNull()", self.location);
}

/// Asserts that actual value is not null.
/// 
/// Example:
/// ```tolk
/// expect(null).toBeNotNull();
/// ```
fun Expectation<T?>.toBeNotNull(self) {
    Assert.notEqual(self.value, null, "expect(actual).toBeNotNull()", self.location);
}

/// Asserts that actual value is not empty.
/// 
/// Example:
/// ```tolk
/// expect("").toBeNonEmpty();
/// ```
fun Expectation<slice>.toBeNonEmpty(self) {
    Assert.notEqual(self.value, "", "expect(actual).toBeNonEmpty()", self.location);
}

/// Asserts that actual value is empty.
/// 
/// Example:
/// ```tolk
/// expect("").toBeEmpty();
/// ```
fun Expectation<slice>.toBeEmpty(self) {
    Assert.equal(self.value, "", "expect(actual).toBeEmpty()", self.location);
}

/// Asserts that actual value is NaN.
/// 
/// Example:
/// ```tolk
/// expect(nan()).toBeNaN();
/// ```
fun Expectation<int>.toBeNaN(self) {
    Assert.equal(self.value, __nan(), "expect(actual).toBeNaN()", self.location);
}

/// Asserts that actual value is not NaN.
/// 
/// Example:
/// ```tolk
/// expect(nan()).toBeNonNaN();
/// ```
fun Expectation<int>.toBeNonNaN(self) {
    Assert.notEqual(self.value, __nan(), "expect(actual).toBeNonNaN()", self.location);
}

/// Asserts that actual value is true.
/// 
/// Example:
/// ```tolk
/// expect(true).toBeTrue();
/// ```
fun Expectation<bool>.toBeTrue(self) {
    Assert.equal(self.value, true, "expect(actual).toBeTrue()", self.location);
}

/// Asserts that actual value is false.
/// 
/// Example:
/// ```tolk
/// expect(false).toBeFalse();
/// ```
fun Expectation<bool>.toBeFalse(self) {
    Assert.equal(self.value, false, "expect(actual).toBeFalse()", self.location);
}

/// Asserts that actual address is internal.
/// 
/// Example:
/// ```tolk
/// expect(addr).toBeInternalAddress();
/// ```
fun Expectation<address>.toBeInternalAddress(self) {
    if (!self.value.isInternal()) {
        Assert.fail("Expected address to be internal", self.location);
    }
}

/// Asserts that actual address is none.
/// 
/// Example:
/// ```tolk
/// expect(addr).toBeNoneAddress();
/// ```
fun Expectation<address>.toBeNoneAddress(self) {
    if (!self.value.isNone()) {
        Assert.fail("Expected address to be none", self.location);
    }
}

/// Asserts that actual address is external.
/// 
/// Example:
/// ```tolk
/// expect(addr).toBeExternalAddress();
/// ```
fun Expectation<address>.toBeExternalAddress(self) {
    if (!self.value.isExternal()) {
        Assert.fail("Expected address to be external", self.location);
    }
}

/// Asserts that actual value represeneted as tuple contains the expected value.
/// 
/// Example:
/// ```tolk
/// var tuple = createEmptyTuple();
/// tuple.push(1);
/// tuple.push(2);
/// tuple.push(3);
/// expect(tuple).toContain(2);
/// ```
fun Expectation<tuple>.toContain<U>(self, expected: U) {
    var i = 0;
    while (i < self.value.size()) {
        val item = self.value.get<U>(i);
        if (item == expected) {
            return;
        }
        i += 1;
    }
    Assert.fail("Tuple doesn't contain the value", self.location);
}

/// Asserts that actual value represeneted as tuple does not contain the expected value.
/// 
/// Example:
/// ```tolk
/// var tuple = createEmptyTuple();
/// tuple.push(1);
/// tuple.push(2);
/// tuple.push(3);
/// expect(tuple).toNotContain(4);
/// ```
fun Expectation<tuple>.toNotContain<U>(self, expected: U) {
    var i = 0;
    while (i < self.value.size()) {
        val item = self.value.get<U>(i);
        if (item == expected) {
            Assert.fail("Tuple contains the value but it should not", self.location);
        }
        i += 1;
    }
}

/// Asserts that actual value represeneted as tuple has the expected length.
/// 
/// Example:
/// ```tolk
/// var tuple = createEmptyTuple();
/// tuple.push(1);
/// tuple.push(2);
/// tuple.push(3);
/// expect(tuple).toHaveLength(3);
fun Expectation<tuple>.toHaveLength(self, length: int) {
    Assert.equal(self.value.size(), length, "expect(actual).toHaveLength(expected)", self.location);
}

/// Creates a new expectation for the given value.
/// 
/// After creation, the expectation can be used to assert the value.
/// 
/// For example:
/// ```tolk
/// expect(10).toEqual(10);
/// ```
/// 
/// Optional location can be provided to specify the location of the expectation to
/// be printed in the error message. You likely don't want to set this manually,
/// as the location will be automatically set by the test framework.
fun expect<T>(value: T, location: slice = "") {
    return Expectation<T>.create(value, __typeName(value), location);
}

/// Returns a NaN value.
/// 
/// Example:
/// ```tolk
/// expect(nan()).toBeNaN();
/// ```
fun __nan(): int
    asm "PUSHNAN"

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
