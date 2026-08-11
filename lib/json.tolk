/// Module for reading values out of JSON documents.
///
/// This module provides a small, read-only JSON decoder for use during contract
/// testing or script execution. It is the natural companion to `fs.readFile`,
/// which loads a JSON file as a `string` but does not parse it.
///
/// Values are addressed with a JSON Pointer (RFC 6901): a `/`-separated path
/// such as `/token/decimals` or `/items/0`. An empty pointer (`""`) selects the
/// whole document.
///
/// Lookup semantics (consistent with `@acton/env`):
/// - Invalid JSON, a missing path, or a type mismatch all return `null`, so `??`
///   can supply a default.
/// - `getInt` accepts a JSON integer, or a JSON string holding a decimal or
///   `0x`-hex integer. Non-integers (floats) return `null`. To carry full
///   257-bit values losslessly, encode the number as a JSON string.
///
/// Examples:
/// ```tolk
/// val src = fs.readFile("fixtures/jetton-metadata.json");
/// if (src != null) {
///     val decimals = json.getInt(src!, "/decimals") ?? 9;
///     val name = json.getString(src!, "/name") ?? "unknown";
///     val mintable = json.getBool(src!, "/mintable") ?? false;
///     if (json.exists(src!, "/admin")) {
///         println("metadata declares an admin");
///     }
/// }
/// ```

/// Namespace for JSON decoding operations.
struct json

/// Reads an integer at the given JSON Pointer.
///
/// Returns:
/// - the value when it is a JSON integer, or a JSON string containing a decimal
///   or `0x`-hex integer.
/// - `null` when the source is invalid JSON, the pointer is absent, or the value
///   is not an integer (for example a float, boolean, object, or array).
///
/// Bare JSON numbers larger than 64 bits are not reliably representable; pass
/// such values as JSON strings to preserve full precision.
fun json.getInt(src: string, path: string): int?
    asm "300 EXTCALL"

/// Reads a string at the given JSON Pointer.
///
/// Returns the value when it is a JSON string, otherwise `null` (invalid JSON,
/// missing pointer, or a non-string value).
fun json.getString(src: string, path: string): string?
    asm "301 EXTCALL"

/// Reads a boolean at the given JSON Pointer.
///
/// Returns the value when it is a JSON boolean, otherwise `null` (invalid JSON,
/// missing pointer, or a non-boolean value). A present `false` is returned as
/// `false`, not `null`.
fun json.getBool(src: string, path: string): bool?
    asm "302 EXTCALL"

/// Checks whether the given JSON Pointer resolves to a value.
///
/// Returns:
/// - `true` when the pointer resolves to any JSON value (including `null`).
/// - `false` when the source is invalid JSON or the pointer is absent.
fun json.exists(src: string, path: string): bool
    asm "303 EXTCALL"
