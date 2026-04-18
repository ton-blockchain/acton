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
/// println("Iteration: {}", 5);
/// println("Transfer {} to {}", amount, myAddress);
/// println("hello", "world");
/// println(1, 2);
///
/// // Error logging
/// eprintln("Critical failure in setup!");
/// ```

import "@stdlib/reflection"
import "ffi"

/// Prints up to 6 values to the standard output.
///
/// If the first argument is a `string`, `println` tries to interpret it as a format string.
/// Placeholder-consumed arguments are rendered into that string, and any remaining arguments
/// are appended separated by spaces. If the string is not a valid format string, it is printed
/// literally and the remaining arguments are still appended separated by spaces.
///
/// Example:
/// ```tolk
/// println(10);
/// println("Hello, world!");
/// println((1, 2, 3));
/// println(Data {
///     value: 10,
///     isSet: true
/// });
///
/// println("Hello, 0x{:x}", 10);
/// println("{} -> {}", "from", "to");
/// println("value {}", 42);
/// println(1, 2);
/// ```
///
/// Format supports the following placeholders when the first argument is a string:
/// - `{}` - default formatter output
/// - `{:x}` - formats an int as lowercase hexadecimal without `0x` prefix
/// - `{:ton}` - formats an int amount in nanotons as `<value> TON`
///
/// Notes:
/// - Escape literal braces with `{{` and `}}`.
/// - If the first string is an invalid format string, it is printed literally.
/// - If placeholders outnumber arguments, unmatched placeholders stay in output as text.
/// - If arguments outnumber placeholders, extra arguments are appended after the formatted text.
/// - For non-int arguments, `{:x}` and `{:ton}` fall back to the same behavior as `{}`.
/// - `{:ton}` uses floating-point conversion, so very large values can be rounded.
fun println<T1, T2 = void, T3 = void, T4 = void, T5 = void, T6 = void>(
    value1: T1,
    value2: T2,
    value3: T3,
    value4: T4,
    value5: T5,
    value6: T6,
): void {
    ffi.println(
        reflect.typeNameOfObject(value1),
        value1.toTuple(),
        reflect.typeNameOfObject(value2),
        value2.toTuple(),
        reflect.typeNameOfObject(value3),
        value3.toTuple(),
        reflect.typeNameOfObject(value4),
        value4.toTuple(),
        reflect.typeNameOfObject(value5),
        value5.toTuple(),
        reflect.typeNameOfObject(value6),
        value6.toTuple(),
    );
}

/// Prints the given string to the standard error output.
///
/// Example:
/// ```tolk
/// eprintln("Something went wrong");
/// ```
fun eprintln(str: string): void {
    ffi.eprintln(str);
}

fun ffi.println(type1: string, arg1: tuple, type2: string, arg2: tuple, type3: string, arg3: tuple, type4: string, arg4: tuple, type5: string, arg5: tuple, type6: string, arg6: tuple): void
    asm "1 EXTCALL"
fun ffi.eprintln(str: string): void
    asm "2 EXTCALL"
