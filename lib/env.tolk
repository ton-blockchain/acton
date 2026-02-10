/// Module for accessing environment variables.
///
/// This module provides a type-safe way to access environment variables during test execution or script running.
/// It supports parsing environment variables into standard Tolk types like `int`, `bool`, `slice`, `address`, and `cell`.
///
/// Examples:
/// ```tolk
/// // Read an integer configuration
/// val iterations = envOr<int>("TEST_ITERATIONS", 100);
///
/// // Read a target address
/// val target = env<address>("TARGET_ADDRESS");
/// if (target == null) {
///     throw 100; // error: target not found
/// }
///
/// // Read a feature flag
/// if (envOr<bool>("ENABLE_LOGGING", false)) {
///     println("Logging enabled");
/// }
/// ```

import "@stdlib/reflection"
import "fmt"
import "ffi/ffi"
import "testing/assert"

/// Returns value of the environment variable with the given name
/// or `null` if not found or type mismatched.
///
/// Supported types:
/// - `int`: parsed as decimal or hexadecimal (if prefixed with `0x`).
/// - `bool`: returns true if value is "true" or "1" (case-insensitive), otherwise false.
/// - `slice`: returns the raw string value.
/// - `address`: parsed as a TON address (raw or user-friendly).
/// - `cell`: parsed from base64 or hex encoded BoC.
///
/// Example:
/// ```tolk
/// val foo = env<int>("FOO");
/// val bar = env<slice>("BAR");
/// ```
fun env<T>(name: string): T? {
    return T.__env(name);
}

/// Returns value of the environment variable with the given name
/// or `default` value if not found or type mismatched.
///
/// Supported types:
/// - `int`: parsed as decimal or hexadecimal (if prefixed with `0x`).
/// - `bool`: returns true if value is "true" or "1" (case-insensitive), otherwise false.
/// - `slice`: returns the raw string value.
/// - `address`: parsed as a TON address (raw or user-friendly).
/// - `cell`: parsed from base64 or hex encoded BoC.
///
/// Example:
/// ```tolk
/// val foo = envOr<int>("FOO", 42);
/// val bar = envOr<slice>("BAR", "default");
/// ```
fun envOr<T>(name: string, default: T): T {
    val value = env<T>(name);
    if (value == null) {
        return default;
    }
    return value;
}

fun int.__env(name: string): int? {
    return ffi.envInt(name);
}

fun bool.__env(name: string): bool? {
    return ffi.envBool(name);
}

fun string.__env(name: string): string? {
    return ffi.envSlice(name);
}

fun address.__env(name: string): address? {
    return ffi.envAddress(name);
}

fun cell.__env(name: string): cell? {
    return ffi.envCell(name);
}

fun T.__env(name: string): T? {
    Assert.fail(format1("env() supports only int, bool, slice, address and cell types, but got ", reflect.typeNameOf<T>()));
}

fun ffi.envInt(name: string): int?
    asm "50 EXTCALL"

fun ffi.envBool(name: string): bool?
    asm "51 EXTCALL"

fun ffi.envSlice(name: string): string?
    asm "52 EXTCALL"

fun ffi.envAddress(name: string): address?
    asm "53 EXTCALL"

fun ffi.envCell(name: string): cell?
    asm "54 EXTCALL"
