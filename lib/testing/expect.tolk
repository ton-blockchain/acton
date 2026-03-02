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

import "@stdlib/reflection"
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
    private valueType: string
    /// The location of the expectation to be printed in the error message.
    /// This field is set by the test framework and should not be set manually.
    private location: string = ""
}

/// Internal function to create a new expectation for the given value.
///
/// Don't use this function directly. Use the `expect` function instead.
///
/// Example:
/// ```tolk
/// expect(10).toEqual(10);
/// ```
fun Expectation<T>.create(value: T, valueType: string, location: string = "") {
    return Expectation<T> {
        value,
        valueType,
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

/// Asserts that actual value is equal to expected value as decimal with given precision.
///
/// Comparison itself is exact (no rounding is applied to actual or expected values).
/// `decimals` controls only failure-message formatting:
/// decimal point insertion, trailing-zero trimming, and normalized integer-style output (`.0`).
///
/// Example:
/// ```tolk
/// expect(ton("1.5")).toEqualDecimal(ton("1.5"), 9);
/// ```
fun Expectation<T>.toEqualDecimal(self, expected: T, decimals: int) {
    Assert.equalDecimal(self.value, expected, decimals, "", self.location);
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
    ffi.assertComparison(
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
    ffi.assertComparison(
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
    ffi.assertComparison(
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
    ffi.assertComparison(
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
fun Expectation<int>.toBeApproxEqAbs(self, expected: int, maxDelta: int) {
    val realDelta = abs(self.value - expected);
    if (realDelta > maxDelta) {
        ffi.assertFail(
            format4(
                "{} !~= {} (max delta: {}, real delta: {})",
                self.value,
                expected,
                maxDelta,
                realDelta
            ),
            self.location
        );
        throw ASSERTION_FAILED;
    }
}

/// Asserts that actual value is approximately equal to expected value using relative percentage difference.
///
/// Relative delta is computed as:
/// `floor(abs(actual - expected) * 100 / abs(actual))`.
/// The assertion passes when this computed percentage is less than or equal to `maxDelta`.
///
/// Zero branch:
/// if `actual == 0`, only `expected == 0` passes; any non-zero `expected` is treated as infinite delta and fails.
///
/// Example:
/// ```tolk
/// expect(1000).toBeApproxEqRel(1099, 9);
/// expect(0).toBeApproxEqRel(0, 0);
/// ```
fun Expectation<int>.toBeApproxEqRel(self, expected: int, maxDelta: int) {
    val valueDelta = abs(self.value - expected);
    val denominator = abs(self.value);
    if (denominator == 0) {
        if (valueDelta > 0) {
            ffi.assertFail(
                format3(
                    "{} !~= {} (max delta: {}%, real delta: inf%)",
                    self.value,
                    expected,
                    maxDelta
                ),
                self.location
            );
            throw ASSERTION_FAILED;
        }
        return;
    }

    val percent = mulDivFloor(valueDelta, 100, denominator);
    if (percent > maxDelta) {
        ffi.assertFail(
            format4(
                "{} !~= {} (max delta: {}%, real delta: {}%)",
                self.value,
                expected,
                maxDelta,
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
    if (self.value != null) {
        Assert.fail("expect(<actual>).toBeNull(): value is not null", self.location);
    }
}

/// Asserts that actual value is not null.
///
/// Example:
/// ```tolk
/// val someValue: int? = cond ? 10 : null;
/// expect(someValue).toBeNotNull();
/// ```
fun Expectation<T?>.toBeNotNull(self) {
    if (self.value == null) {
        Assert.fail("expect(<actual>).toBeNotNull(): value is null", self.location);
    }
}

/// Asserts that actual `Maybe<T>` value is defined (`Some`).
///
/// Fails when actual value is `None`.
///
/// Example:
/// ```tolk
/// val someValue = Maybe<int>.some(10);
/// expect(someValue).toBeDefined();
/// ```
fun Expectation<Maybe<T>>.toBeDefined(self) {
    if (self.value is None) {
        Assert.fail("expect(<actual>).toBeDefined()", self.location);
    }
}

/// Asserts that actual `Maybe<T>` value is `None`.
///
/// Fails when actual value is `Some`.
///
/// Example:
/// ```tolk
/// val noneValue = Maybe<int>.none();
/// expect(noneValue).toBeNone();
/// ```
fun Expectation<Maybe<T>>.toBeNone(self) {
    if (self.value !is None) {
        Assert.fail("expect(<actual>).toBeNone()", self.location);
    }
}

/// Asserts that actual value is not empty.
///
/// Example:
/// ```tolk
/// expect("hello").toBeNonEmpty();
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
/// expect(0).toBeNonNaN();
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

/// Asserts that actual value represeneted as an array contains the expected value.
///
/// Example:
/// ```tolk
/// val arr = [1, 2, 3];
/// expect(arr).toContain(2);
/// ```
fun Expectation<array<E>>.toContain<U>(self, expected: U) {
    var i = 0;
    while (i < self.value.size()) {
        val item = self.value.get(i) as U;
        if (item == expected) {
            return;
        }
        i += 1;
    }
    Assert.fail("Array doesn't contain the value", self.location);
}

/// Asserts that actual value represeneted as an array does not contain the expected value.
///
/// Example:
/// ```tolk
/// val arr = [1, 2, 3];
/// expect(arr).toNotContain(4);
/// ```
fun Expectation<array<E>>.toNotContain<U>(self, expected: U) {
    var i = 0;
    while (i < self.value.size()) {
        val item = self.value.get(i) as U;
        if (item == expected) {
            Assert.fail("Array contains the value, but it should not", self.location);
        }
        i += 1;
    }
}

/// Asserts that actual value represeneted as an array is empty.
///
/// Example:
/// ```tolk
/// val arr = array<int> [];
/// expect(arr).toBeEmpty();
/// ```
fun Expectation<array<E>>.toBeEmpty(self) {
    if (self.value.size() != 0) {
        Assert.fail("expect(<actual>).toBeEmpty(<expected>): array is not empty", self.location);
    }
}

/// Asserts that actual value represeneted as an array is not empty.
///
/// Example:
/// ```tolk
/// val arr = [1, 2, 3];
/// expect(arr).toBeNonEmpty();
/// ```
fun Expectation<array<E>>.toBeNonEmpty(self) {
    if (self.value.size() == 0) {
        Assert.fail("expect(<actual>).toBeNonEmpty(<expected>): array is empty", self.location);
    }
}

/// Asserts that actual value represeneted as an array has the expected length.
///
/// Example:
/// ```tolk
/// val arr = [1, 2, 3];
/// expect(arr).toHaveLength(3);
/// ```
fun Expectation<array<E>>.toHaveLength(self, length: int) {
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
        val mapValue = r.loadValue();
        if (mapValue == value) {
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
        val mapValue = r.loadValue();
        if (mapValue == value) {
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
    var mapLength = 0;
    var r = self.value.findFirst();
    while (r.isFound) {
        mapLength += 1;
        r = self.value.iterateNext(r)
    }

    Assert.equal(length, mapLength, "expect(<actual>).toHaveLength(<expected>)", self.location)
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
fun expect<T>(value: T, loc: SourceLocation = reflect.sourceLocation()) {
    val locString = format3("{}:{}:{}", loc.fileName, loc.lineNo, loc.charNo);
    return Expectation<T>.create(value, reflect.typeNameOfObject(value), locString);
}

/// Sets the expected exit code for the test.
///
/// Example:
/// ```tolk
/// expectToEndWithExitCode(10);
/// throw 10; // Test will end with exit code 10 and be considered as passed
/// ```
///
/// Alternatively, you can use `@test({ fail_with: 10 })` annotation to set
/// the expected exit code for the whole test.
///
/// ```tolk
/// @test({ fail_with: 10 })
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
fun ffi.assertComparison<V1, V2>(left: V1, right: V2, op: string, message: string, location: string) {
    if (
        !ffi.assertBin(
            op,
            reflect.typeNameOfObject(left),
            left.toTuple(),
            reflect.typeNameOfObject(right),
            right.toTuple(),
            message,
            location
        )
    ) {
        throw ASSERTION_FAILED;
    }
}
