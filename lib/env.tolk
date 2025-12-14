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
fun env<T>(name: slice): T? {
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
fun envOr<T>(name: slice, default: T): T {
    val value = env<T>(name);
    if (value == null) {
        return default;
    }
    return value;
}

fun int.__env(name: slice): int? {
    return ffi.env_int(name);
}

fun bool.__env(name: slice): bool? {
    return ffi.env_bool(name);
}

fun slice.__env(name: slice): slice? {
    return ffi.env_slice(name);
}

fun address.__env(name: slice): address? {
    return ffi.env_address(name);
}

fun cell.__env(name: slice): cell? {
    return ffi.env_cell(name);
}

fun T.__env(name: slice): T? {
    Assert.fail(format1("env() supports only int, bool, slice, address and cell types, but got ", T.typeName()));
}

fun ffi.env_int(name: slice): int?
    asm "50 EXTCALL"

fun ffi.env_bool(name: slice): bool?
    asm "51 EXTCALL"

fun ffi.env_slice(name: slice): slice?
    asm "52 EXTCALL"

fun ffi.env_address(name: slice): address?
    asm "53 EXTCALL"

fun ffi.env_cell(name: slice): cell?
    asm "54 EXTCALL"
