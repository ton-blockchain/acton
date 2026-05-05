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
import "../ffi"
import "../fmt"
import "../impl"
import "../types/big_array"

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
    private valueType: int
    /// The location of the expectation to be printed in the error message.
    /// This field is set by the test framework and should not be set manually.
    private location: string
}

/// Internal function to create a new expectation for the given value.
///
/// Don't use this function directly. Use the `expect` function instead.
///
/// Example:
/// ```tolk
/// expect(10).toEqual(10);
/// ```
fun Expectation<T>.create(value: T, valueType: int, location: string): Expectation<T> {
    return Expectation<T> { value, valueType, location };
}

/// Fails this expectation at its location.
/// A convenient wrapper to be used in `Expectation<T>.xxx` methods instead of `Assert.fail`.
///
/// Example:
/// ```tolk
/// fun Expectation<int>.toBeTen(self) {
///     if (self.value != 10) {
///         self.fail("integer is not 10");
///     }
/// }
/// ```
fun Expectation<T>.fail(self, message: string): never {
    Assert.fail(message, self.location);
}

/// Asserts that actual value is equal to expected value.
///
/// Example:
/// ```tolk
/// expect(10).toEqual(10);
/// ```
fun Expectation<T>.toEqual<U>(self, expected: U): void {
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
fun Expectation<T>.toEqualDecimal(self, expected: T, decimals: int): void {
    Assert.equalDecimal(self.value, expected, decimals, "", self.location);
}

/// Asserts that actual value is not equal to expected value.
///
/// Example:
/// ```tolk
/// expect(10).toNotEqual(20);
/// ```
fun Expectation<T>.toNotEqual<U>(self, expected: U): void {
    Assert.notEqual(self.value, expected, "expect(<actual>).toNotEqual(<expected>)", self.location);
}

/// Asserts that actual value is less than expected value.
///
/// Example:
/// ```tolk
/// expect(5).toBeLess(10);
/// ```
fun Expectation<T>.toBeLess<U>(self, expected: U): void {
    ffi.assertComparison(
        "<",
        self.value,
        expected,
        "expect(<actual>).toBeLess(<expected>)",
        self.location,
    );
}

/// Asserts that actual value is greater than expected value.
///
/// Example:
/// ```tolk
/// expect(10).toBeGreater(5);
/// ```
fun Expectation<T>.toBeGreater<U>(self, expected: U): void {
    ffi.assertComparison(
        ">",
        self.value,
        expected,
        "expect(<actual>).toBeGreater(<expected>)",
        self.location,
    );
}

/// Asserts that actual value is less than or equal to expected value.
///
/// Example:
/// ```tolk
/// expect(5).toBeLessOrEqual(10);
/// expect(10).toBeLessOrEqual(10);
/// ```
fun Expectation<T>.toBeLessOrEqual<U>(self, expected: U): void {
    ffi.assertComparison(
        "<=",
        self.value,
        expected,
        "expect(<actual>).toBeLessOrEqual(<expected>)",
        self.location,
    );
}

/// Asserts that actual value is greater than or equal to expected value.
///
/// Example:
/// ```tolk
/// expect(10).toBeGreaterOrEqual(5);
/// expect(10).toBeGreaterOrEqual(10);
/// ```
fun Expectation<T>.toBeGreaterOrEqual<U>(self, expected: U): void {
    ffi.assertComparison(
        ">=",
        self.value,
        expected,
        "expect(<actual>).toBeGreaterOrEqual(<expected>)",
        self.location,
    );
}

/// Asserts that actual value is approximately equal to expected value using absolute difference.
///
/// Example:
/// ```tolk
/// expect(10).toBeApproxEqAbs(15, 10); // ok, difference is 5, which is less than maximum allowed difference of 10
/// expect(10).toBeApproxEqAbs(20, 5); // error, difference is 10, which is greater than maximum allowed difference of 5
/// ```
fun Expectation<int>.toBeApproxEqAbs(self, expected: int, maxDelta: int): void {
    val realDelta = abs(self.value - expected);
    if (realDelta > maxDelta) {
        self.fail(
            format(
                "{} !~= {} (max delta: {}, real delta: {})",
                self.value,
                expected,
                maxDelta,
                realDelta,
            ),
        );
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
fun Expectation<int>.toBeApproxEqRel(self, expected: int, maxDelta: int): void {
    val valueDelta = abs(self.value - expected);
    val denominator = abs(self.value);
    if (denominator == 0) {
        if (valueDelta > 0) {
            self.fail(
                format(
                    "{} !~= {} (max delta: {}%, real delta: inf%)",
                    self.value,
                    expected,
                    maxDelta,
                ),
            );
        }
        return;
    }

    val percent = mulDivFloor(valueDelta, 100, denominator);
    if (percent > maxDelta) {
        self.fail(
            format(
                "{} !~= {} (max delta: {}%, real delta: {}%)",
                self.value,
                expected,
                maxDelta,
                percent,
            ),
        );
    }
}

/// Asserts that actual value is null.
///
/// Example:
/// ```tolk
/// expect(null).toBeNull();
/// ```
fun Expectation<T?>.toBeNull(self): void {
    if (self.value != null) {
        self.fail("expect(<actual>).toBeNull(): value is not null");
    }
}

/// Asserts that actual value is not null.
///
/// Example:
/// ```tolk
/// val someValue: int? = cond ? 10 : null;
/// expect(someValue).toBeNotNull();
/// ```
fun Expectation<T?>.toBeNotNull(self): void {
    if (self.value == null) {
        self.fail("expect(<actual>).toBeNotNull(): value is null");
    }
}

/// Asserts that actual value is not empty (contains some bits of refs).
///
/// Example:
/// ```tolk
/// expect(someSlice).toBeNonEmpty();
/// ```
fun Expectation<slice>.toBeNonEmpty(self): void {
    if (self.value.isEmpty()) {
        self.fail("expect(<actual>).toBeNonEmpty()");
    }
}

/// Asserts that actual value is empty (contains 0 bits and 0 refs).
///
/// Example:
/// ```tolk
/// expect(someSlice).toBeEmpty();
/// ```
fun Expectation<slice>.toBeEmpty(self): void {
    if (!self.value.isEmpty()) {
        self.fail("expect(<actual>).toBeEmpty()");
    }
}

/// Asserts that actual value is NaN.
///
/// Example:
/// ```tolk
/// expect(nan()).toBeNaN();
/// ```
fun Expectation<int>.toBeNaN(self): void {
    Assert.equal(self.value, impl.nan(), "expect(<actual>).toBeNaN()", self.location);
}

/// Asserts that actual value is not NaN.
///
/// Example:
/// ```tolk
/// expect(0).toBeNonNaN();
/// ```
fun Expectation<int>.toBeNonNaN(self): void {
    Assert.notEqual(self.value, impl.nan(), "expect(<actual>).toBeNonNaN()", self.location);
}

/// Asserts that actual value is true.
///
/// Example:
/// ```tolk
/// expect(true).toBeTrue();
/// ```
fun Expectation<bool>.toBeTrue(self): void {
    Assert.equal(self.value, true, "expect(<actual>).toBeTrue()", self.location);
}

/// Asserts that actual value is false.
///
/// Example:
/// ```tolk
/// expect(false).toBeFalse();
/// ```
fun Expectation<bool>.toBeFalse(self): void {
    Assert.equal(self.value, false, "expect(<actual>).toBeFalse()", self.location);
}

/// Asserts that actual address is internal.
///
/// Example:
/// ```tolk
/// expect(addr).toBeInternalAddress();
/// ```
fun Expectation<any_address>.toBeInternalAddress(self): void {
    if (!self.value.isInternal()) {
        self.fail("expect(<actual>).toBeInternalAddress(): Actual address is not internal");
    }
}

/// Asserts that actual address is none.
///
/// Example:
/// ```tolk
/// expect(addr).toBeNoneAddress();
/// ```
fun Expectation<any_address>.toBeNoneAddress(self): void {
    if (!self.value.isNone()) {
        self.fail("expect(<actual>).toBeNoneAddress(): Actual address is not none");
    }
}

/// Asserts that actual address is external.
///
/// Example:
/// ```tolk
/// expect(addr).toBeExternalAddress();
/// ```
fun Expectation<any_address>.toBeExternalAddress(self): void {
    if (!self.value.isExternal()) {
        self.fail("expect(<actual>).toBeExternalAddress(): Actual address is not external");
    }
}

/// Asserts that an array contains the expected value.
///
/// Example:
/// ```tolk
/// val arr = [1, 2, 3];
/// expect(arr).toContain(2);
/// ```
fun Expectation<array<E>>.toContain<U>(self, expected: U): void {
    var i = 0;
    repeat (self.value.size()) {
        val item = self.value.get(i) as U;
        if (item == expected) {
            return;
        }
        i += 1;
    }
    self.fail("Array doesn't contain the value");
}

/// Asserts that an array does not contain the expected value.
///
/// Example:
/// ```tolk
/// val arr = [1, 2, 3];
/// expect(arr).toNotContain(4);
/// ```
fun Expectation<array<E>>.toNotContain<U>(self, expected: U): void {
    var i = 0;
    repeat (self.value.size()) {
        val item = self.value.get(i) as U;
        if (item == expected) {
            self.fail("Array contains the value, but it should not");
        }
        i += 1;
    }
}

/// Asserts that an array is empty.
///
/// Example:
/// ```tolk
/// val arr = array<int> [];
/// expect(arr).toBeEmpty();
/// ```
fun Expectation<array<E>>.toBeEmpty(self): void {
    if (self.value.size() != 0) {
        self.fail("expect(<actual>).toBeEmpty(<expected>): array is not empty");
    }
}

/// Asserts that an array is empty.
///
/// Example:
/// ```tolk
/// val txs = net.send(deployer.address, msg);
/// expect(txs).toBeEmpty();
/// ```
fun Expectation<BigArray<E>>.toBeEmpty(self): void {
    if (self.value.size() != 0) {
        self.fail("expect(<actual>).toBeEmpty(<expected>): array is not empty");
    }
}

/// Asserts that an array is not empty.
///
/// Example:
/// ```tolk
/// val arr = [1, 2, 3];
/// expect(arr).toBeNonEmpty();
/// ```
fun Expectation<array<E>>.toBeNonEmpty(self): void {
    if (self.value.size() == 0) {
        self.fail("expect(<actual>).toBeNonEmpty(<expected>): array is empty");
    }
}

/// Asserts that an array is not empty.
///
/// Example:
/// ```tolk
/// val txs = net.send(deployer.address, msg);
/// expect(txs).toBeNonEmpty();
/// ```
fun Expectation<BigArray<E>>.toBeNonEmpty(self): void {
    if (self.value.size() == 0) {
        self.fail("expect(<actual>).toBeNonEmpty(<expected>): array is empty");
    }
}

/// Asserts that an array has the expected length.
///
/// Example:
/// ```tolk
/// val arr = [1, 2, 3];
/// expect(arr).toHaveLength(3);
/// ```
fun Expectation<array<E>>.toHaveLength(self, length: int): void {
    Assert.equal(
        self.value.size(),
        length,
        "expect(<actual>).toHaveLength(<expected>)",
        self.location,
    );
}

/// Asserts that an array has the expected length.
///
/// Example:
/// ```tolk
/// val txs = net.send(deployer.address, msg);
/// expect(txs).toHaveLength(3);
/// ```
fun Expectation<BigArray<E>>.toHaveLength(self, length: int): void {
    Assert.equal(
        self.value.size(),
        length,
        "expect(<actual>).toHaveLength(<expected>)",
        self.location,
    );
}

/// Asserts that a map contains the expected key.
///
/// Example:
/// ```tolk
/// var map = map<int32, int32> [];
/// map.set(1, 2);
/// map.set(2, 3);
/// expect(map).toContainKey(2);
/// ```
fun Expectation<map<K, V>>.toContainKey(self, key: K): void {
    val result = self.value.get(key);
    if (!result.isFound) {
        self.fail(
            format(
                "expect(<actual>).toContainKey(<expected>): Key '{}' not found in the map",
                key,
            ),
        );
    }
}

/// Asserts that actual value represeneted as map does NOT contain the expected key.
///
/// Example:
/// ```tolk
/// var map = map<int32, int32> [];
/// map.set(1, 2);
/// map.set(2, 3);
/// expect(map).toNotContainKey(4);
/// ```
fun Expectation<map<K, V>>.toNotContainKey(self, key: K): void {
    val result = self.value.get(key);
    if (result.isFound) {
        self.fail(
            format(
                "expect(<actual>).toNotContainKey(<expected>): Key '{}' is found in the map",
                key,
            ),
        );
    }
}

/// Asserts that actual value represeneted as map contains the expected value.
///
/// Example:
/// ```tolk
/// var map = map<int32, int32> [];
/// map.set(1, 2);
/// map.set(2, 3);
/// expect(map).toContainValue(2);
/// ```
fun Expectation<map<K, V>>.toContainValue(self, value: V): void {
    var r = self.value.findFirst();
    while (r.isFound) {
        val mapValue = r.loadValue();
        if (mapValue == value) {
            return;
        }
        r = self.value.iterateNext(r);
    }

    self.fail(
        format(
            "expect(<actual>).toContainValue(<expected>): Value '{}' not found in the map",
            value,
        ),
    );
}

/// Asserts that actual value represeneted as map does NOT contain the expected value.
///
/// Example:
/// ```tolk
/// var map = map<int32, int32> [];
/// map.set(1, 2);
/// map.set(2, 3);
/// expect(map).toNotContainValue(4);
/// ```
fun Expectation<map<K, V>>.toNotContainValue(self, value: V): void {
    var r = self.value.findFirst();
    while (r.isFound) {
        val mapValue = r.loadValue();
        if (mapValue == value) {
            self.fail(
                format(
                    "expect(<actual>).toNotContainValue(<expected>): Value '{}' is found in the map",
                    value,
                ),
            );
        }
        r = self.value.iterateNext(r);
    }
}

/// Asserts that actual value represeneted as map is empty.
///
/// Example:
/// ```tolk
/// var map = map<int32, int32> [];
/// expect(map).toBeEmpty();
/// ```
fun Expectation<map<K, V>>.toBeEmpty(self): void {
    if (!self.value.isEmpty()) {
        self.fail("expect(<actual>).toBeEmpty(): Map is not empty");
    }
}

/// Asserts that actual value represeneted as map is not empty.
///
/// Example:
/// ```tolk
/// var map = map<int32, int32> [];
/// map.set(1, 2);
/// map.set(2, 3);
/// expect(map).toBeNonEmpty();
/// ```
fun Expectation<map<K, V>>.toBeNonEmpty(self): void {
    if (self.value.isEmpty()) {
        self.fail("expect(<actual>).toBeNonEmpty(): Map is empty");
    }
}

/// Asserts that actual value represeneted as map has the expected length.
///
/// Example:
/// ```tolk
/// var map = map<int32, int32> [];
/// map.set(1, 2);
/// map.set(2, 3);
/// expect(map).toHaveLength(2);
/// ```
fun Expectation<map<K, V>>.toHaveLength(self, length: int): void {
    var mapLength = 0;
    var r = self.value.findFirst();
    while (r.isFound) {
        mapLength += 1;
        r = self.value.iterateNext(r);
    }

    Assert.equal(mapLength, length, "expect(<actual>).toHaveLength(<expected>)", self.location);
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
fun expect<T>(value: T, loc: string = reflect.sourceLocationAsString()): Expectation<T> {
    return Expectation<T>.create(
        value,
        reflect.typeUniqueIdxOfObject(value),
        loc,
    );
}

/// Sets the expected exit code for the test.
///
/// Example:
/// ```tolk
/// expectToEndWithExitCode(10);
/// throw 10; // Test will end with exit code 10 and be considered as passed
/// ```
///
/// Alternatively, you can use `@test.fail_with(10)` annotation to set
/// the expected exit code for the whole test.
///
/// ```tolk
/// @test.fail_with(10)
/// get fun `test fail with`() {
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
/// expect(impl.nan()).toBeNaN();
/// ```
fun impl.nan(): int
    asm "PUSHNAN"
