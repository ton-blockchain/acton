/// Module for high-level assertion API.
///
/// This module provides a fluent "expect" style API for writing test assertions.
/// It wraps the lower-level `Assert` functions and provides a more readable syntax.
/// It supports a wide range of types including integers, booleans, slices, addresses, tuples, maps, and optionals.
///
/// Examples:
/// ```tolk
/// // Simple value check
/// expect(total).toEqual(100);
///
/// // Comparison
/// expect(balance).toBeGreater(ton("1.0"));
///
/// // Approximate check (useful for gas calculations)
/// expect(gasUsed).toBeApproxEqRel(expectedGas, 10); // within 10%
///
/// // Map check
/// expect(balances).toContainKey(userAddress);
/// expect(balances.get(userAddress)).toEqual(ton("50"));
/// ```

import "assert"
import "../fmt"
import "../ffi/ffi"
import "../tlb/maybe"

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
    Assert.equal(self.value, expected, "expect(<actual>).toEqual(<expected>)", self.location);
}

/// Asserts that actual value is not equal to expected value.
/// 
/// Example:
/// ```tolk
/// expect(10).toNotEqual(20);
/// ```
fun Expectation<T>.toNotEqual<U>(self, expected: U) {
    Assert.notEqual(self.value, expected, "expect(<actual>).toNotEqual(<expected>)", self.location);
}

/// Asserts that actual value is less than expected value.
///
/// Example:
/// ```tolk
/// expect(5).toBeLess(10);
/// ```
fun Expectation<T>.toBeLess<U>(self, expected: U) {
    ffi.assert_comparison(
        self.value,
        expected,
        "<",
        "expect(<actual>).toBeLess(<expected>)",
        self.location
    );
}

/// Asserts that actual value is greater than expected value.
///
/// Example:
/// ```tolk
/// expect(10).toBeGreater(5);
/// ```
fun Expectation<T>.toBeGreater<U>(self, expected: U) {
    ffi.assert_comparison(
        self.value,
        expected,
        ">",
        "expect(<actual>).toBeGreater(<expected>)",
        self.location
    );
}

/// Asserts that actual value is less than or equal to expected value.
///
/// Example:
/// ```tolk
/// expect(5).toBeLessOrEqual(10);
/// expect(10).toBeLessOrEqual(10);
/// ```
fun Expectation<T>.toBeLessOrEqual<U>(self, expected: U) {
    ffi.assert_comparison(
        self.value,
        expected,
        "<=",
        "expect(<actual>).toBeLessOrEqual(<expected>)",
        self.location
    );
}

/// Asserts that actual value is greater than or equal to expected value.
///
/// Example:
/// ```tolk
/// expect(10).toBeGreaterOrEqual(5);
/// expect(10).toBeGreaterOrEqual(10);
/// ```
fun Expectation<T>.toBeGreaterOrEqual<U>(self, expected: U) {
    ffi.assert_comparison(
        self.value,
        expected,
        ">=",
        "expect(<actual>).toBeGreaterOrEqual(<expected>)",
        self.location
    );
}

/// Asserts that actual value is approximately equal to expected value using absolute difference.
/// 
/// Example:
/// ```tolk
/// expect(10).toBeApproxEqAbs(15, 10); // ok, difference is 5, which is less than maximum allowed difference of 10
/// expect(10).toBeApproxEqAbs(20, 5); // error, difference is 10, which is greater than maximum allowed difference of 5
/// ```
fun Expectation<int>.toBeApproxEqAbs(self, expected: int, max_delta: int) {
    val real_delta = abs(self.value - expected);
    if (real_delta > max_delta) {
        ffi.assert_fail(
            format4(
                "{} !~= {} (max delta: {}, real delta: {}",
                self.value,
                expected,
                max_delta,
                real_delta
            ),
            self.location
        );
        throw ASSERTION_FAILED;
    }
}

/// Asserts that actual value is approximately equal to expected value using difference in percentage.
/// 
/// Example:
/// ```tolk
/// expect(10).toBeApproxEqRel(15, 60); // ok, difference is 50%, which is less than maximum allowed difference of 60%
/// expect(10).toBeApproxEqRel(20, 50); // error, difference is 100%, which is greater than maximum allowed difference of 50%
/// ```
fun Expectation<int>.toBeApproxEqRel(self, expected: int, max_delta: int) {
    val value_delta = abs(self.value - expected);
    val percent = mulDivFloor(value_delta, 100, self.value);
    if (percent > max_delta) {
        ffi.assert_fail(
            format4(
                "{} !~= {} (max delta: {}%, real delta: {}%",
                self.value,
                expected,
                max_delta,
                percent
            ),
            self.location
        );
        throw ASSERTION_FAILED;
    }
}

/// Asserts that actual value is null.
/// 
/// Example:
/// ```tolk
/// expect(null).toBeNull();
/// ```
fun Expectation<T?>.toBeNull(self) {
    Assert.equal(self.value, null, "expect(<actual>).toBeNull()", self.location);
}

/// Asserts that actual value is not null.
/// 
/// Example:
/// ```tolk
/// expect(null).toBeNotNull();
/// ```
fun Expectation<T?>.toBeNotNull(self) {
    Assert.notEqual(self.value, null, "expect(<actual>).toBeNotNull()", self.location);
}

fun Expectation<Maybe<T>>.toBeDefined(self) {
    if (self.value is None) {
        Assert.fail("expect(<actual>).toBeDefined()", self.location);
    }
}
fun Expectation<Maybe<T>>.toBeNone(self) {
    if (self.value !is None) {
        Assert.fail("expect(<actual>).toBeNone()", self.location);
    }
}

/// Asserts that actual value is not empty.
/// 
/// Example:
/// ```tolk
/// expect("").toBeNonEmpty();
/// ```
fun Expectation<slice>.toBeNonEmpty(self) {
    Assert.notEqual(self.value, "", "expect(<actual>).toBeNonEmpty()", self.location);
}

/// Asserts that actual value is empty.
/// 
/// Example:
/// ```tolk
/// expect("").toBeEmpty();
/// ```
fun Expectation<slice>.toBeEmpty(self) {
    Assert.equal(self.value, "", "expect(<actual>).toBeEmpty()", self.location);
}

/// Asserts that actual value is NaN.
/// 
/// Example:
/// ```tolk
/// expect(nan()).toBeNaN();
/// ```
fun Expectation<int>.toBeNaN(self) {
    Assert.equal(self.value, ffi.nan(), "expect(<actual>).toBeNaN()", self.location);
}

/// Asserts that actual value is not NaN.
/// 
/// Example:
/// ```tolk
/// expect(nan()).toBeNonNaN();
/// ```
fun Expectation<int>.toBeNonNaN(self) {
    Assert.notEqual(self.value, ffi.nan(), "expect(<actual>).toBeNonNaN()", self.location);
}

/// Asserts that actual value is true.
/// 
/// Example:
/// ```tolk
/// expect(true).toBeTrue();
/// ```
fun Expectation<bool>.toBeTrue(self) {
    Assert.equal(self.value, true, "expect(<actual>).toBeTrue()", self.location);
}

/// Asserts that actual value is false.
/// 
/// Example:
/// ```tolk
/// expect(false).toBeFalse();
/// ```
fun Expectation<bool>.toBeFalse(self) {
    Assert.equal(self.value, false, "expect(<actual>).toBeFalse()", self.location);
}

/// Asserts that actual address is internal.
/// 
/// Example:
/// ```tolk
/// expect(addr).toBeInternalAddress();
/// ```
fun Expectation<any_address>.toBeInternalAddress(self) {
    if (!self.value.isInternal()) {
        Assert.fail("expect(<actual>).toBeInternalAddress(): Expected address to be internal", self.location);
    }
}

/// Asserts that actual address is none.
/// 
/// Example:
/// ```tolk
/// expect(addr).toBeNoneAddress();
/// ```
fun Expectation<any_address>.toBeNoneAddress(self) {
    if (!self.value.isNone()) {
        Assert.fail("expect(<actual>).toBeNoneAddress(): Expected address to be none", self.location);
    }
}

/// Asserts that actual address is external.
/// 
/// Example:
/// ```tolk
/// expect(addr).toBeExternalAddress();
/// ```
fun Expectation<any_address>.toBeExternalAddress(self) {
    if (!self.value.isExternal()) {
        Assert.fail("expect(<actual>).toBeExternalAddress(): Expected address to be external", self.location);
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
/// ```
fun Expectation<tuple>.toHaveLength(self, length: int) {
    Assert.equal(self.value.size(), length, "expect(<actual>).toHaveLength(<expected>)", self.location);
}

/// Asserts that actual value represeneted as map contains the expected key.
/// 
/// Example:
/// ```tolk
/// var map = createEmptyMap<int32, int32>();
/// map.set(1, 2);
/// map.set(2, 3);
/// expect(map).toContainKey(2);
/// ```
fun Expectation<map<K, V>>.toContainKey(self, key: K) {
    val result = self.value.get(key);
    if (!result.isFound) {
        Assert.fail(format1("expect(<actual>).toContainKey(<expected>): Key '{}' not found in the map", key), self.location)
    }
}

/// Asserts that actual value represeneted as map does NOT contain the expected key.
/// 
/// Example:
/// ```tolk
/// var map = createEmptyMap<int32, int32>();
/// map.set(1, 2);
/// map.set(2, 3);
/// expect(map).toNotContainKey(4);
/// ```
fun Expectation<map<K, V>>.toNotContainKey(self, key: K) {
    val result = self.value.get(key);
    if (result.isFound) {
        Assert.fail(format1("expect(<actual>).toNotContainKey(<expected>): Key '{}' is found in the map", key), self.location)
    }
}

/// Asserts that actual value represeneted as map contains the expected value.
/// 
/// Example:
/// ```tolk
/// var map = createEmptyMap<int32, int32>();
/// map.set(1, 2);
/// map.set(2, 3);
/// expect(map).toContainValue(2);
/// ```
fun Expectation<map<K, V>>.toContainValue(self, value: V) {
    var r = self.value.findFirst();
    while (r.isFound) {
        val map_value = r.loadValue();
        if (map_value == value) {
            return;
        }
        r = self.value.iterateNext(r)
    }

    Assert.fail(format1("expect(<actual>).toContainValue(<expected>): Value '{}' not found in the map", value), self.location)
}

/// Asserts that actual value represeneted as map does NOT contain the expected value.
/// 
/// Example:
/// ```tolk
/// var map = createEmptyMap<int32, int32>();
/// map.set(1, 2);
/// map.set(2, 3);
/// expect(map).toNotContainValue(4);
/// ```
fun Expectation<map<K, V>>.toNotContainValue(self, value: V) {
    var r = self.value.findFirst();
    while (r.isFound) {
        val map_value = r.loadValue();
        if (map_value == value) {
            Assert.fail(format1("expect(<actual>).toNotContainValue(<expected>): Value '{}' is found in the map", value), self.location)
        }
        r = self.value.iterateNext(r)
    }
}

/// Asserts that actual value represeneted as map is empty.
/// 
/// Example:
/// ```tolk
/// var map = createEmptyMap<int32, int32>();
/// expect(map).toBeEmpty();
/// ```
fun Expectation<map<K, V>>.toBeEmpty(self) {
    if (!self.value.isEmpty()) {
        Assert.fail("expect(<actual>).toBeEmpty(): Map is not empty", self.location)
    }
}

/// Asserts that actual value represeneted as map is not empty.
/// 
/// Example:
/// ```tolk
/// var map = createEmptyMap<int32, int32>();
/// map.set(1, 2);
/// map.set(2, 3);
/// expect(map).toBeNonEmpty();
/// ```
fun Expectation<map<K, V>>.toBeNonEmpty(self) {
    if (self.value.isEmpty()) {
        Assert.fail("expect(<actual>).toBeNonEmpty(): Map is empty", self.location)
    }
}

/// Asserts that actual value represeneted as map has the expected length.
/// 
/// Example:
/// ```tolk
/// var map = createEmptyMap<int32, int32>();
/// map.set(1, 2);
/// map.set(2, 3);
/// expect(map).toHaveLength(2);
/// ```
fun Expectation<map<K, V>>.toHaveLength(self, length: int) {
    var map_length = 0;
    var r = self.value.findFirst();
    while (r.isFound) {
        map_length += 1;
        r = self.value.iterateNext(r)
    }

    Assert.equal(length, map_length, "expect(<actual>).toHaveLength(<expected>)", self.location)
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
    return Expectation<T>.create(value, value.typeNameOfObject(), location);
}

/// Sets the expected exit code for the test.
/// 
/// Example:
/// ```tolk
/// expectToEndWithExitCode(10);
/// throw 10; // Test will end with exit code 10 and be considered as passed
/// ```
/// 
/// Alternatively, you can use `@custom({ fail_with: 10 })` annotation to set 
/// the expected exit code for the whole test.
/// 
/// ```tolk
/// @custom({ fail_with: 10 })
/// get fun test_fail_with() {
///     throw 10; // ok
/// }
/// ```
/// 
/// This function is more flexible and can be used conditionally in the test.
fun expectToEndWithExitCode(code: int): void
    asm "102 EXTCALL"

/// Returns a NaN value.
/// 
/// Example:
/// ```tolk
/// expect(ffi.nan()).toBeNaN();
/// ```
fun ffi.nan(): int
    asm "PUSHNAN"

/// Internal helper function for comparison assertions.
///
/// Don't use this function directly. Use comparison methods instead.
fun ffi.assert_comparison<V1, V2>(left: V1, right: V2, op: slice, message: slice, location: slice) {
    if (
        !ffi.assert_bin(
            op,
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
