// Synthetic declarations for compiler-provided symbols that are absent from common.tolk.
// They are indexed by the language server and are never passed to the compiler.

@pure
fun `-_`(x: int): int builtin

@pure
fun `+_`(x: int): int builtin

@pure
fun `!_`(x: int): int builtin

@pure
fun `!b_`(x: bool): bool builtin

@pure
fun `~_`(x: int): int builtin

@pure
fun `_+_`(x: int, y: int): int builtin

@pure
fun `_-_`(x: int, y: int): int builtin

@pure
fun `_*_`(x: int, y: int): int builtin

@pure
fun `_/_`(x: int, y: int): int builtin

@pure
fun `_~/_`(x: int, y: int): int builtin

@pure
fun `_^/_`(x: int, y: int): int builtin

@pure
fun `_%_`(x: int, y: int): int builtin

@pure
fun `_<<_`(x: int, y: int): int builtin

@pure
fun `_>>_`(x: int, y: int): int builtin

@pure
fun `_~>>_`(x: int, y: int): int builtin

@pure
fun `_^>>_`(x: int, y: int): int builtin

@pure
fun `_&_`(x: int, y: int): int builtin

@pure
fun `_|_`(x: int, y: int): int builtin

@pure
fun `_^_`(x: int, y: int): int builtin

@pure
fun `_==_`(x: int, y: int): bool builtin

@pure
fun `_!=_`(x: int, y: int): bool builtin

@pure
fun `_<_`(x: int, y: int): bool builtin

@pure
fun `_>_`(x: int, y: int): bool builtin

@pure
fun `_<=_`(x: int, y: int): bool builtin

@pure
fun `_>=_`(x: int, y: int): bool builtin

@pure
fun `_<=>_`(x: int, y: int): int builtin

@pure
fun __expect_type<T>(actual: T, expected: string) builtin

@pure
fun __expect_lazy<T>(value: T) builtin

@pure
fun __expect_inline<T>(value: T) builtin
