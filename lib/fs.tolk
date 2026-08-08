/// Module for file system operations.
///
/// This module provides a simple interface for reading files from the local file system.
/// This is particularly useful for loading test fixtures, configuration files, or external data
/// needed during contract testing or script execution.
///
/// Examples:
/// ```tolk
/// // Load a JSON configuration
/// val configJson = fs.readFile("config/test-settings.json");
/// if (configJson != null) {
///     // process json...
/// }
///
/// // Load a BoC file as a slice (note: readFile returns string, so binary
/// // data needs specific handling if not hex/base64 text)
/// // Typically used for text-based formats.
/// val contractCodeHex = fs.readFile("build/contract.hex");
/// ```

/// Namespace for file system operations.
struct fs

/// Reads the entire contents of a file at the given path.
///
/// Behavior details:
/// - The file is read as UTF-8 text.
/// - On any read failure, returns `null` (missing file, non-UTF-8 content,
///   permission error, or other host I/O error).
/// - Failure reasons are intentionally collapsed to `null`; no error reason is exposed.
/// - Relative paths are resolved under the Acton project root. Path forms such as
///   `./a.txt` and `dir/../a.txt` are accepted when they stay inside the project.
/// - Absolute paths, parent-directory escapes, and symlinks that resolve outside the project
///   are rejected and return `null`.
/// - Newline bytes are preserved as-is after UTF-8 decoding (for example, CRLF is not
///   normalized to LF).
///
/// Example:
/// ```tolk
/// val content = fs.readFile("config.txt");
/// if (content != null) {
///     println("File content: {}", content);
/// } else {
///     println("File not found");
/// }
/// ```
fun fs.readFile(path: string): string?
    asm "3 EXTCALL"

/// Reads the entire contents of a file as raw bytes.
///
/// Returns:
/// - `slice` with file bytes on success.
/// - `null` on read failure (missing file, permission error, etc).
/// - `null` when the path is absolute or escapes the project root.
fun fs.readBytes(path: string): slice?
    asm "4 EXTCALL"

/// Writes UTF-8 text to a file.
///
/// Returns:
/// - `true` if the file was written successfully.
/// - `false` on any host I/O error.
/// - `false` when the path is absolute or escapes the project root.
fun fs.writeString(path: string, data: string): bool
    asm "5 EXTCALL"

/// Writes raw bytes from a slice to a file.
///
/// Returns:
/// - `true` if the file was written successfully.
/// - `false` on invalid byte slice input or host I/O error.
/// - `false` when the path is absolute or escapes the project root.
fun fs.writeBytes(path: string, data: slice): bool
    asm "7 EXTCALL"

/// Checks whether a path exists.
///
/// Returns:
/// - `true` when the path exists.
/// - `false` when missing or if existence check fails.
/// - `false` when the path is absolute or escapes the project root.
fun fs.exists(path: string): bool
    asm "22 EXTCALL"
