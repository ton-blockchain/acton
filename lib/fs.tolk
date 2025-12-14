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
/// // Load a BoC file as a slice (note: readFile returns string, so binary data needs specific handling if not hex/base64 text)
/// // Typically used for text-based formats.
/// val contractCodeHex = fs.readFile("build/contract.hex");
/// ```

/// Namespace for file system operations.
struct fs

/// Reads the entire contents of a file at the given path as a string slice
/// with snake-string encoding.
///
/// Returns the file contents as a slice if the file exists and can be read,
/// or `null` if the file doesn't exist or cannot be read.
///
/// Example:
/// ```tolk
/// val content = fs.readFile("config.txt");
/// if (content != null) {
///     println1("File content: {}", content);
/// } else {
///     println("File not found");
/// }
/// ```
fun fs.readFile(path: slice): slice? asm "3 EXTCALL"
