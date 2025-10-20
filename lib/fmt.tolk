
/// Formats a string with 1 argument.
///
/// Example:
/// ```tolk
/// let msg = format1("Hello {}", "world");
/// println(msg);  // prints "Hello world"
/// ```
fun format1<T1>(fmt: slice, arg1: T1): slice {
    return __format1_impl(fmt, __typeName(arg1), arg1.__toTuple());
}

/// Formats a string with 2 arguments.
///
/// Example:
/// ```tolk
/// let msg = format2("{}: {}", "name", "John");
/// println(msg);  // prints "name: John"
/// ```
fun format2<T1, T2>(fmt: slice, arg1: T1, arg2: T2): slice {
    return __format2_impl(fmt, __typeName(arg1), arg1.__toTuple(), __typeName(arg2), arg2.__toTuple());
}

/// Formats a string with 3 arguments.
///
/// Example:
/// ```tolk
/// let msg = format3("{} + {} = {}", 2, 3, 5);
/// println(msg);  // prints "2 + 3 = 5"
/// ```
fun format3<T1, T2, T3>(fmt: slice, arg1: T1, arg2: T2, arg3: T3): slice {
    return __format3_impl(fmt, __typeName(arg1), arg1.__toTuple(), __typeName(arg2), arg2.__toTuple(), __typeName(arg3), arg3.__toTuple());
}

/// Formats a string with 4 arguments.
///
/// Example:
/// ```tolk
/// let msg = format4("{} {} {} {}", "a", "b", "c", "d");
/// println(msg);  // prints "a b c d"
/// ```
fun format4<T1, T2, T3, T4>(fmt: slice, arg1: T1, arg2: T2, arg3: T3, arg4: T4): slice {
    return __format4_impl(fmt, __typeName(arg1), arg1.__toTuple(), __typeName(arg2), arg2.__toTuple(), __typeName(arg3), arg3.__toTuple(), __typeName(arg4), arg4.__toTuple());
}

/// Formats a string with 5 arguments.
///
/// Example:
/// ```tolk
/// let msg = format5("{} {} {} {} {}", 1, 2, 3, 4, 5);
/// println(msg);  // prints "1 2 3 4 5"
/// ```
fun format5<T1, T2, T3, T4, T5>(fmt: slice, arg1: T1, arg2: T2, arg3: T3, arg4: T4, arg5: T5): slice {
    return __format5_impl(fmt, __typeName(arg1), arg1.__toTuple(), __typeName(arg2), arg2.__toTuple(), __typeName(arg3), arg3.__toTuple(), __typeName(arg4), arg4.__toTuple(), __typeName(arg5), arg5.__toTuple());
}

fun __format1_impl(fmt: slice, type1: slice, arg1: tuple): slice
    asm "200 EXTCALL"
fun __format2_impl(fmt: slice, type1: slice, arg1: tuple, type2: slice, arg2: tuple): slice
    asm "201 EXTCALL"
fun __format3_impl(fmt: slice, type1: slice, arg1: tuple, type2: slice, arg2: tuple, type3: slice, arg3: tuple): slice
    asm "202 EXTCALL"
fun __format4_impl(fmt: slice, type1: slice, arg1: tuple, type2: slice, arg2: tuple, type3: slice, arg3: tuple, type4: slice, arg4: tuple): slice
    asm "203 EXTCALL"
fun __format5_impl(fmt: slice, type1: slice, arg1: tuple, type2: slice, arg2: tuple, type3: slice, arg3: tuple, type4: slice, arg4: tuple, type5: slice, arg5: tuple): slice
    asm "204 EXTCALL"
