/// Module for string formatting.
///
/// This module provides a set of functions (`format1` to `format5`) for creating formatted strings.
/// It is similar to `sprintf` in C or `format!` in Rust.
///
/// Usage:
/// The functions are named `formatN` where N is the number of arguments.
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
/// // Multiple arguments
/// val info = format3("User {} has {} items worth {:ton}", "Alice", 5, 1000000000);
/// ```

import "@stdlib/reflection"
import "ffi/ffi"

/// Formats a string with 1 argument.
///
/// Example:
/// ```tolk
/// val msg = format1("Hello {}", "world");
/// println(msg);  // prints "Hello world"
/// ```
///
/// Format supports following format soecifiers:
/// - `{:x}` — formats a number as hexadecimal without `0x` prefix
/// - `{:ton}` — formats a number as TON value (e.g `0.11 TON`)
fun format1<T1>(fmt: string, arg1: T1): string {
    return ffi.format1(fmt, reflect.typeNameOfObject(arg1), arg1.toTuple());
}

/// Formats a string with 2 arguments.
///
/// Example:
/// ```tolk
/// val msg = format2("{}: {}", "name", "John");
/// println(msg);  // prints "name: John"
/// ```
///
/// Format supports following format soecifiers:
/// - `{:x}` — formats a number as hexadecimal without `0x` prefix
/// - `{:ton}` — formats a number as TON value (e.g `0.11 TON`)
fun format2<T1, T2>(fmt: string, arg1: T1, arg2: T2): string {
    return ffi.format2(fmt, reflect.typeNameOfObject(arg1), arg1.toTuple(), reflect.typeNameOfObject(arg2), arg2.toTuple());
}

/// Formats a string with 3 arguments.
///
/// Example:
/// ```tolk
/// val msg = format3("{} + {} = {}", 2, 3, 5);
/// println(msg);  // prints "2 + 3 = 5"
/// ```
///
/// Format supports following format soecifiers:
/// - `{:x}` — formats a number as hexadecimal without `0x` prefix
/// - `{:ton}` — formats a number as TON value (e.g `0.11 TON`)
fun format3<T1, T2, T3>(fmt: string, arg1: T1, arg2: T2, arg3: T3): string {
    return ffi.format3(fmt, reflect.typeNameOfObject(arg1), arg1.toTuple(), reflect.typeNameOfObject(arg2), arg2.toTuple(), reflect.typeNameOfObject(arg3), arg3.toTuple());
}

/// Formats a string with 4 arguments.
///
/// Example:
/// ```tolk
/// val msg = format4("{} {} {} {}", "a", "b", "c", "d");
/// println(msg);  // prints "a b c d"
/// ```
///
/// Format supports following format soecifiers:
/// - `{:x}` — formats a number as hexadecimal without `0x` prefix
/// - `{:ton}` — formats a number as TON value (e.g `0.11 TON`)
fun format4<T1, T2, T3, T4>(fmt: string, arg1: T1, arg2: T2, arg3: T3, arg4: T4): string {
    return ffi.format4(fmt, reflect.typeNameOfObject(arg1), arg1.toTuple(), reflect.typeNameOfObject(arg2), arg2.toTuple(), reflect.typeNameOfObject(arg3), arg3.toTuple(), reflect.typeNameOfObject(arg4), arg4.toTuple());
}

/// Formats a string with 5 arguments.
///
/// Example:
/// ```tolk
/// val msg = format5("{} {} {} {} {}", 1, 2, 3, 4, 5);
/// println(msg);  // prints "1 2 3 4 5"
/// ```
///
/// Format supports following format soecifiers:
/// - `{:x}` — formats a number as hexadecimal without `0x` prefix
/// - `{:ton}` — formats a number as TON value (e.g `0.11 TON`)
fun format5<T1, T2, T3, T4, T5>(fmt: string, arg1: T1, arg2: T2, arg3: T3, arg4: T4, arg5: T5): string {
    return ffi.format5(fmt, reflect.typeNameOfObject(arg1), arg1.toTuple(), reflect.typeNameOfObject(arg2), arg2.toTuple(), reflect.typeNameOfObject(arg3), arg3.toTuple(), reflect.typeNameOfObject(arg4), arg4.toTuple(), reflect.typeNameOfObject(arg5), arg5.toTuple());
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
