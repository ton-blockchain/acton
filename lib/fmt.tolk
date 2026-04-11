/// Module for string formatting.
///
/// This module provides a universal `format` function for creating formatted strings.
/// It accepts up to 5 typed arguments.
///
/// Supported placeholders:
/// - `{}`: default formatting
/// - `{:x}`: integer formatted as hexadecimal without `0x`
/// - `{:ton}`: integer nanotons formatted as `<value> TON`
///
/// Format rules and edge cases:
/// - Escape literal braces with `{{` and `}}`.
/// - Extra arguments are ignored.
/// - Missing arguments leave placeholders unchanged in output (`{}`, `{:x}`, `{:ton}`).
/// - Invalid format strings (unknown modifier, unsupported placeholder, unmatched braces)
///   fail at runtime with a parser error that includes a byte offset.
/// - For non-int arguments, `{:x}` and `{:ton}` fall back to default formatting.
/// - `{:ton}` uses `f64` internally, so very large values can lose precision.
///
/// Examples:
/// ```tolk
/// // Simple string substitution
/// val greeting = format("Hello, {}!", "Tolk");
///
/// // Formatting numbers
/// val hex = format("Value in hex: 0x{:x}", 255); // "Value in hex: 0xff"
///
/// // Formatting TON amounts
/// val balance = format("Balance: {:ton}", 500_000_000); // "Balance: 0.5 TON"
///
/// // Escaping braces
/// val escaped = format("Use {{}} to print braces", 0); // "Use {} to print braces"
///
/// // Missing argument keeps placeholder as-is
/// val partial = format("{} {}", "left"); // "left {}"
///
/// // Multiple arguments
/// val info = format("User {} has {} items worth {:ton}", "Alice", 5, 1_000_000_000);
/// ```

import "@stdlib/reflection"
import "ffi/ffi"

/// Formats a string with up to 5 arguments.
///
/// Uses the module-level format rules, including escaping (`{{`/`}}`), mismatch behavior,
/// and runtime parser errors for invalid format strings.
///
/// Example:
/// ```tolk
/// val a = format("Hello {}", "world");
/// val b = format("{} + {} = {}", 2, 3, 5);
/// val c = format("hello");
/// ```
///
/// Non-int fallback examples:
/// ```tolk
/// format("{:x}", "abc");   // "abc"
/// format("{:ton}", true);  // "true"
/// ```
fun format<T1 = void, T2 = void, T3 = void, T4 = void, T5 = void>(
    fmt: string,
    arg1: T1,
    arg2: T2,
    arg3: T3,
    arg4: T4,
    arg5: T5,
): string {
    return ffi.format(
        fmt,
        reflect.typeNameOfObject(arg1),
        arg1.toTuple(),
        reflect.typeNameOfObject(arg2),
        arg2.toTuple(),
        reflect.typeNameOfObject(arg3),
        arg3.toTuple(),
        reflect.typeNameOfObject(arg4),
        arg4.toTuple(),
        reflect.typeNameOfObject(arg5),
        arg5.toTuple(),
    );
}

fun ffi.format(fmt: string, type1: string, arg1: tuple, type2: string, arg2: tuple, type3: string, arg3: tuple, type4: string, arg4: tuple, type5: string, arg5: tuple): string
    asm "200 EXTCALL"
