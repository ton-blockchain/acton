/// Module for fuzz-testing helpers.
///
/// This module contains helpers intended specifically for parameterized tests
/// annotated with `@test({ fuzz: ... })`.
///
/// Examples:
/// ```tolk
/// import "@acton/testing/expect"
/// import "@acton/testing/fuzz"
///
/// @test({ fuzz: true })
/// get fun `test balance stays bounded`(value: int) {
///     val bounded = fuzz.bound(value, 0, 100);
///     fuzz.assume(bounded != 13);
///     expect(bounded >= 0).toBeTrue();
/// }
/// ```

import "@stdlib/reflection"
import "../fmt"
import "assert"

/// Root module for fuzz-testing helpers.
///
/// This structure groups fuzz-specific helpers under a
/// dedicated namespace.
struct fuzz

/// Rejects the current fuzz input when condition is false.
///
/// Outside fuzz tests this is reported as a regular failure.
fun fuzz.assume(
    condition: bool,
    message: string = "",
    loc: SourceLocation = reflect.sourceLocation(),
): void {
    Assert.assume(condition, message, formatFuzzLocation(loc));
}

/// Maps an integer-like value into the inclusive range `[minValue, maxValue]`.
///
/// Values already in range stay unchanged, while out-of-range values wrap back
/// into the target interval.
fun fuzz.bound<T>(
    value: T,
    minValue: T,
    maxValue: T,
    loc: SourceLocation = reflect.sourceLocation(),
): T {
    val minInt = minValue as int;
    val maxInt = maxValue as int;
    if (minInt > maxInt) {
        Assert.fail(
            format(
                "fuzz.bound(<value>, <min>, <max>): min {} must be less than or equal to max {}",
                minInt,
                maxInt,
            ),
            formatFuzzLocation(loc),
        );
    }

    val valueInt = value as int;
    if (valueInt >= minInt && valueInt <= maxInt) {
        return value;
    }

    val size = maxInt - minInt + 1;
    if (valueInt < minInt) {
        val diff = minInt - valueInt;
        val rem = diff % size;
        if (rem == 0) {
            return minValue;
        }
        return (maxInt - rem + 1) as T;
    }

    val diff = valueInt - maxInt;
    val rem = diff % size;
    if (rem == 0) {
        return maxValue;
    }
    return (minInt + rem - 1) as T;
}

fun formatFuzzLocation(loc: SourceLocation): string {
    return format("{}:{}:{}", loc.fileName, loc.lineNo, loc.charNo);
}
