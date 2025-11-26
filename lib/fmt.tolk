import "ffi/ffi"

/// Formats a string with 1 argument.
///
/// Example:
/// ```tolk
/// let msg = format1("Hello {}", "world");
/// println(msg);  // prints "Hello world"
/// ```
///
/// Format supports following format soecifiers:
/// - `{:x}` — formats a number as hexadecimal without `0x` prefix
/// - `{:ton}` — formats a number as TON value (e.g `0.11 TON`)
fun format1<T1>(fmt: slice, arg1: T1): slice {
    return ffi.format1(fmt, arg1.typeNameOfObject(), arg1.toTuple());
}

/// Formats a string with 2 arguments.
///
/// Example:
/// ```tolk
/// let msg = format2("{}: {}", "name", "John");
/// println(msg);  // prints "name: John"
/// ```
///
/// Format supports following format soecifiers:
/// - `{:x}` — formats a number as hexadecimal without `0x` prefix
/// - `{:ton}` — formats a number as TON value (e.g `0.11 TON`)
fun format2<T1, T2>(fmt: slice, arg1: T1, arg2: T2): slice {
    return ffi.format2(fmt, arg1.typeNameOfObject(), arg1.toTuple(), arg2.typeNameOfObject(), arg2.toTuple());
}

/// Formats a string with 3 arguments.
///
/// Example:
/// ```tolk
/// let msg = format3("{} + {} = {}", 2, 3, 5);
/// println(msg);  // prints "2 + 3 = 5"
/// ```
///
/// Format supports following format soecifiers:
/// - `{:x}` — formats a number as hexadecimal without `0x` prefix
/// - `{:ton}` — formats a number as TON value (e.g `0.11 TON`)
fun format3<T1, T2, T3>(fmt: slice, arg1: T1, arg2: T2, arg3: T3): slice {
    return ffi.format3(fmt, arg1.typeNameOfObject(), arg1.toTuple(), arg2.typeNameOfObject(), arg2.toTuple(), arg3.typeNameOfObject(), arg3.toTuple());
}

/// Formats a string with 4 arguments.
///
/// Example:
/// ```tolk
/// let msg = format4("{} {} {} {}", "a", "b", "c", "d");
/// println(msg);  // prints "a b c d"
/// ```
///
/// Format supports following format soecifiers:
/// - `{:x}` — formats a number as hexadecimal without `0x` prefix
/// - `{:ton}` — formats a number as TON value (e.g `0.11 TON`)
fun format4<T1, T2, T3, T4>(fmt: slice, arg1: T1, arg2: T2, arg3: T3, arg4: T4): slice {
    return ffi.format4(fmt, arg1.typeNameOfObject(), arg1.toTuple(), arg2.typeNameOfObject(), arg2.toTuple(), arg3.typeNameOfObject(), arg3.toTuple(), arg4.typeNameOfObject(), arg4.toTuple());
}

/// Formats a string with 5 arguments.
///
/// Example:
/// ```tolk
/// let msg = format5("{} {} {} {} {}", 1, 2, 3, 4, 5);
/// println(msg);  // prints "1 2 3 4 5"
/// ```
///
/// Format supports following format soecifiers:
/// - `{:x}` — formats a number as hexadecimal without `0x` prefix
/// - `{:ton}` — formats a number as TON value (e.g `0.11 TON`)
fun format5<T1, T2, T3, T4, T5>(fmt: slice, arg1: T1, arg2: T2, arg3: T3, arg4: T4, arg5: T5): slice {
    return ffi.format5(fmt, arg1.typeNameOfObject(), arg1.toTuple(), arg2.typeNameOfObject(), arg2.toTuple(), arg3.typeNameOfObject(), arg3.toTuple(), arg4.typeNameOfObject(), arg4.toTuple(), arg5.typeNameOfObject(), arg5.toTuple());
}

fun ffi.format1(fmt: slice, type1: slice, arg1: tuple): slice
    asm "200 EXTCALL"
fun ffi.format2(fmt: slice, type1: slice, arg1: tuple, type2: slice, arg2: tuple): slice
    asm "201 EXTCALL"
fun ffi.format3(fmt: slice, type1: slice, arg1: tuple, type2: slice, arg2: tuple, type3: slice, arg3: tuple): slice
    asm "202 EXTCALL"
fun ffi.format4(fmt: slice, type1: slice, arg1: tuple, type2: slice, arg2: tuple, type3: slice, arg3: tuple, type4: slice, arg4: tuple): slice
    asm "203 EXTCALL"
fun ffi.format5(fmt: slice, type1: slice, arg1: tuple, type2: slice, arg2: tuple, type3: slice, arg3: tuple, type4: slice, arg4: tuple, type5: slice, arg5: tuple): slice
    asm "204 EXTCALL"
