/// Module for string formatting.
///
/// This module provides functions (`format1` to `format5`) for creating formatted strings.
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
/// val greeting = format1("Hello, {}!", "Tolk");
///
/// // Formatting numbers
/// val hex = format1("Value in hex: 0x{:x}", 255); // "Value in hex: 0xff"
///
/// // Formatting TON amounts
/// val balance = format1("Balance: {:ton}", 500000000); // "Balance: 0.5 TON"
///
/// // Escaping braces
/// val escaped = format1("Use {{}} to print braces", 0); // "Use {} to print braces"
///
/// // Missing argument keeps placeholder as-is
/// val partial = format1("{} {}", "left"); // "left {}"
///
/// // Multiple arguments
/// val info = format3("User {} has {} items worth {:ton}", "Alice", 5, 1000000000);
/// ```

import "@stdlib/reflection"
import "ffi/ffi"

/// Formats a string with 1 argument.
///
/// Uses the module-level format rules, including escaping (`{{`/`}}`), mismatch behavior,
/// and runtime parser errors for invalid format strings.
///
/// Example:
/// ```tolk
/// val msg = format1("Hello {}", "world");
/// println(msg);  // prints "Hello world"
/// ```
///
/// Non-int fallback examples:
/// ```tolk
/// format1("{:x}", "abc");   // "abc"
/// format1("{:ton}", true);  // "true"
/// ```
fun format1<T1>(fmt: string, arg1: T1): string {
    return ffi.format1(fmt, reflect.typeNameOfObject(arg1), arg1.toTuple());
}

/// Formats a string with 2 arguments.
///
/// Uses the same placeholder rules and edge-case behavior as [format1].
///
/// Example:
/// ```tolk
/// val msg = format2("{}: {}", "name", "John");
/// println(msg);  // prints "name: John"
/// ```
fun format2<T1, T2>(fmt: string, arg1: T1, arg2: T2): string {
    return ffi.format2(
        fmt,
        reflect.typeNameOfObject(arg1),
        arg1.toTuple(),
        reflect.typeNameOfObject(arg2),
        arg2.toTuple(),
    );
}

/// Formats a string with 3 arguments.
///
/// Uses the same placeholder rules and edge-case behavior as [format1].
///
/// Example:
/// ```tolk
/// val msg = format3("{} + {} = {}", 2, 3, 5);
/// println(msg);  // prints "2 + 3 = 5"
/// ```
fun format3<T1, T2, T3>(fmt: string, arg1: T1, arg2: T2, arg3: T3): string {
    return ffi.format3(
        fmt,
        reflect.typeNameOfObject(arg1),
        arg1.toTuple(),
        reflect.typeNameOfObject(arg2),
        arg2.toTuple(),
        reflect.typeNameOfObject(arg3),
        arg3.toTuple(),
    );
}

/// Formats a string with 4 arguments.
///
/// Uses the same placeholder rules and edge-case behavior as [format1].
///
/// Example:
/// ```tolk
/// val msg = format4("{} {} {} {}", "a", "b", "c", "d");
/// println(msg);  // prints "a b c d"
/// ```
fun format4<T1, T2, T3, T4>(fmt: string, arg1: T1, arg2: T2, arg3: T3, arg4: T4): string {
    return ffi.format4(
        fmt,
        reflect.typeNameOfObject(arg1),
        arg1.toTuple(),
        reflect.typeNameOfObject(arg2),
        arg2.toTuple(),
        reflect.typeNameOfObject(arg3),
        arg3.toTuple(),
        reflect.typeNameOfObject(arg4),
        arg4.toTuple(),
    );
}

/// Formats a string with 5 arguments.
///
/// Uses the same placeholder rules and edge-case behavior as [format1].
///
/// Example:
/// ```tolk
/// val msg = format5("{} {} {} {} {}", 1, 2, 3, 4, 5);
/// println(msg);  // prints "1 2 3 4 5"
/// ```
fun format5<T1, T2, T3, T4, T5>(
    fmt: string,
    arg1: T1,
    arg2: T2,
    arg3: T3,
    arg4: T4,
    arg5: T5,
): string {
    return ffi.format5(
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

/// Stores the decimal string representation of an integer into a builder.
///
/// Example:
/// ```tolk
/// var b = beginCell();
/// b.storeIntAsString(42); // stores "42" as ASCII bytes
/// ```
fun builder.storeIntAsString(mutate self, x: int): void asm """
    ZERO SWAP

    UNTIL:<{
        10 PUSHINT DIVMOD
        48 ADDCONST
        s2 XCHG0
        s2 s3 XCHG
        INC SWAP
        DUP ISZERO
    }> DROP

    REPEAT:<{ 8 STU }>
""";

/// Converts an integer to its decimal string representation.
///
/// Example:
/// ```tolk
/// val s = 42.toString(); // "42"
/// val zero = 0.toString(); // "0"
/// ```
fun int.toString(self): string {
    var b = beginCell();
    b.storeIntAsString(self);
    return b.toSlice();
}

fun ffi.format1(fmt: string, type1: string, arg1: tuple): string
    asm "200 EXTCALL"
fun ffi.format2(fmt: string, type1: string, arg1: tuple, type2: string, arg2: tuple): string
    asm "201 EXTCALL"
fun ffi.format3(fmt: string, type1: string, arg1: tuple, type2: string, arg2: tuple, type3: string, arg3: tuple): string
    asm "202 EXTCALL"
fun ffi.format4(fmt: string, type1: string, arg1: tuple, type2: string, arg2: tuple, type3: string, arg3: tuple, type4: string, arg4: tuple): string
    asm "203 EXTCALL"
fun ffi.format5(fmt: string, type1: string, arg1: tuple, type2: string, arg2: tuple, type3: string, arg3: tuple, type4: string, arg4: tuple, type5: string, arg5: tuple): string
    asm "204 EXTCALL"
