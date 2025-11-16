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
    ffi.println(value.typeNameOfObject(), value.toTuple());
}

/// Prints the given slice string to the standard error output.
/// 
/// Example:
/// ```tolk
/// eprintln("Something went wrong");
/// ```
fun eprintln(str: slice): void {
    ffi.eprintln(str);
}

fun ffi.println(type_name: slice, str: tuple): void
    asm "1 EXTCALL"
fun ffi.eprintln(str: slice): void
    asm "2 EXTCALL"
