/// Module for input/output operations.
///
/// This module provides functions for printing to the standard output (stdout) and standard error (stderr).
/// It includes helper functions for formatted output, making it easier to debug contracts and scripts.
///
/// Examples:
/// ```tolk
/// // Simple debug print
/// println("Starting test...");
///
/// // Print structural data
/// val point = Point { x: 10, y: 20 };
/// println(point); // Output: Point { x: 10, y: 20 }
///
/// // Formatted output
/// println1("Iteration: {}", 5);
/// println1("Address: {}", myAddress);
///
/// // Error logging
/// eprintln("Critical failure in setup!");
/// ```

import "@stdlib/reflection"
import "fmt"
import "ffi/ffi"

/// Prints the given value to the standard output.
///
/// Example:
/// ```tolk
/// println(10);
/// println("Hello, world!");
/// println((1, 2, 3));
/// println(Data {
///     value: 10,
///     is_set: true
/// });
/// ```
fun println<T>(value: T): void {
    ffi.println(reflect.typeNameOfObject(value), value.toTuple());
}

/// Prints the given value to the standard output formatted according to format string.
///
/// Example:
/// ```tolk
/// println1("Hello, 0x{:x}", 10);
/// ```
///
/// Format supports the following placeholders:
/// - `{}` - default formatter output
/// - `{:x}` - formats an int as lowercase hexadecimal without `0x` prefix
/// - `{:ton}` - formats an int amount in nanotons as `<value> TON`
///
/// Notes:
/// - Escape literal braces with `{{` and `}}`.
/// - Invalid format strings fail at runtime with parser errors that include byte positions
///   (for example: unknown modifiers, unsupported placeholder payload, unclosed `{`,
///   unmatched `}`).
/// - If placeholders outnumber arguments, unmatched placeholders stay in output as text.
/// - If arguments outnumber placeholders, extra arguments are ignored.
/// - For non-int arguments, `{:x}` and `{:ton}` fall back to the same behavior as `{}`.
/// - `{:ton}` uses floating-point conversion, so very large values can be rounded.
fun println1<T>(fmt: string, value: T): void {
    println(format1(fmt, value));
}

/// Prints the given slice string to the standard error output.
///
/// Example:
/// ```tolk
/// eprintln("Something went wrong");
/// ```
fun eprintln(str: string): void {
    ffi.eprintln(str);
}

fun ffi.println(typeName: string, str: tuple): void
    asm "1 EXTCALL"
fun ffi.eprintln(str: string): void
    asm "2 EXTCALL"
