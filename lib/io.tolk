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
/// - `{:X}` - same lowercase hexadecimal as `{:x}`, but with `0x` prefix
/// - `{:b}` - formats an int as binary without `0b` prefix
/// - `{:B}` - same binary as `{:b}`, but with `0b` prefix
/// - `{:gram}` / `{:grams}` - formats an int amount in nanograms as `<value> GRAM`
/// - `{:ton}` - legacy alias for `{:gram}` / `{:grams}`
/// - `{:cell-tree}` - formats a cell/slice/builder/Cell<T> as a tree of cell references
///
/// Placeholder syntax is the same as `format` from `fmt.tolk`:
/// - The full format spec is made of optional fill, align, width, and type parts.
/// - `{:>8}`, `{:_<8}`, and `{:^8}` set constant width and alignment.
/// - `{:0>8x}` and `{:0>8:x}` both combine fill, width, and a type modifier.
/// - `{:05}` is sign-aware zero padding; `{:08X}` pads after the `0x` prefix.
/// - Width is minimum visible width; longer values are not truncated.
/// - `X` and `B` only add prefixes; they do not uppercase digits.
/// - Dynamic width and named arguments are not supported.
///
/// Notes:
/// - Escape literal braces with `{{` and `}}`.
/// - If the first string is an invalid format string, it is printed literally.
/// - If placeholders outnumber arguments, the full unmatched placeholders stay in output as text.
/// - If arguments outnumber placeholders, extra arguments are appended after the formatted text.
/// - For non-int arguments, `{:x}`, `{:X}`, `{:b}`, `{:B}`, `{:gram}`, `{:grams}`,
///   and `{:ton}` fall back to the same behavior as `{}`.
/// - For non-cell/slice/builder/Cell<T> arguments, `{:cell-tree}` falls back to the same behavior as `{}`.
fun println<
    T1,
    T2 = void,
    T3 = void,
    T4 = void,
    T5 = void,
    T6 = void,
>(
    value1: T1,
    value2: T2,
    value3: T3,
    value4: T4,
    value5: T5,
    value6: T6,
): void {
    ffi.println(
        reflect.typeUniqueIdxOfObject(value1),
        value1.toTuple(),
        reflect.typeUniqueIdxOfObject(value2),
        value2.toTuple(),
        reflect.typeUniqueIdxOfObject(value3),
        value3.toTuple(),
        reflect.typeUniqueIdxOfObject(value4),
        value4.toTuple(),
        reflect.typeUniqueIdxOfObject(value5),
        value5.toTuple(),
        reflect.typeUniqueIdxOfObject(value6),
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

fun ffi.println(
    type1: int,
    arg1: tuple,
    type2: int,
    arg2: tuple,
    type3: int,
    arg3: tuple,
    type4: int,
    arg4: tuple,
    type5: int,
    arg5: tuple,
    type6: int,
    arg6: tuple,
): void
    asm "1 EXTCALL"

fun ffi.eprintln(str: string): void
    asm "2 EXTCALL"
