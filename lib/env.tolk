/// Module for accessing environment variables.
///
/// This module provides a type-safe way to access environment variables during test execution or script running.
/// It supports parsing environment variables into standard Tolk types like `int`, `coins`, `bool`,
/// `string`, `slice`, `address`, and `cell`.
///
/// `env<T>` supports only these types. For unsupported generic `T`, execution fails
/// via [Assert.fail].
///
/// Note for `bool`: when a variable is present, only `"true"` and `"1"` (case-insensitive) map
/// to `true`; any other present value maps to `false`.
///
/// Examples:
/// ```tolk
/// // Read an integer configuration
/// val iterations = env<int>("TEST_ITERATIONS") ?? 100;
///
/// // Read TON amount in nanotons
/// val minBalance = env<coins>("MIN_BALANCE") ?? ton("0.5");
///
/// // Read a target address
/// val target = env<address>("TARGET_ADDRESS");
/// if (target == null) {
///     throw 100; // error: target not found
/// }
///
/// // Read a feature flag
/// if (env<bool>("ENABLE_LOGGING") ?? false) {
///     println("Logging enabled");
/// }
/// ```

import "@stdlib/reflection"
import "fmt"
import "ffi"
import "testing/assert"

/// Returns value of the environment variable with the given name
/// or `null` if not found or parsing fails for a supported type.
///
/// Supported types:
/// - `int`: parsed as decimal or hexadecimal (if prefixed with `0x`).
/// - `coins`: parsed the same way as `int` and returned as TON amount in nanotons.
/// - `bool`: for present variable, returns true if value is "true" or "1" (case-insensitive), otherwise false.
/// - `string`: returns the raw string value.
/// - `slice`: returns the raw string value as a slice.
/// - `address`: parsed as a TON address (raw or user-friendly).
/// - `cell`: parsed from base64 or hex encoded BoC.
///
/// Unsupported generic types do not return `null` and instead fail via [Assert.fail].
///
/// Example:
/// ```tolk
/// val foo = env<int>("FOO") ?? 42;
/// val amount = env<coins>("TRANSFER_VALUE") ?? ton("0.05");
/// val bar = env<string>("BAR") ?? "default";
/// ```
fun env<T>(name: string): T? {
    return T.__env(name);
}

fun int.__env(name: string): int? {
    return ffi.envInt(name);
}

fun coins.__env(name: string): coins? {
    val value = ffi.envInt(name);
    if (value == null) {
        return null
    }
    return value;
}

fun bool.__env(name: string): bool? {
    return ffi.envBool(name);
}

fun string.__env(name: string): string? {
    return ffi.envString(name);
}

fun slice.__env(name: string): slice? {
    return ffi.envSlice(name);
}

fun address.__env(name: string): address? {
    return ffi.envAddress(name);
}

fun cell.__env(name: string): cell? {
    return ffi.envCell(name);
}

fun T.__env(name: string): T? {
    Assert.fail(
        format(
            "env() supports only int, coins, bool, string, slice, address and cell types, but got {}",
            reflect.typeNameOf<T>(),
        ),
    );
}

fun ffi.envInt(name: string): int?
    asm "50 EXTCALL"

fun ffi.envBool(name: string): bool?
    asm "51 EXTCALL"

fun ffi.envString(name: string): string?
    asm "52 EXTCALL"

fun ffi.envSlice(name: string): slice?
    asm "55 EXTCALL"

fun ffi.envAddress(name: string): address?
    asm "53 EXTCALL"

fun ffi.envCell(name: string): cell?
    asm "54 EXTCALL"
