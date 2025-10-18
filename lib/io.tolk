fun println<T>(value: T): void {
    println_impl(__typeName(value), value.__toTuple())
}
fun println_impl(type_name: slice, str: tuple): void asm "1 EXTCALL"
fun eprintln(str: slice): void asm "2 EXTCALL"
