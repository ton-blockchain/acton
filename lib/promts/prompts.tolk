/// Module for user prompts.
///
/// This module enables interactive scripts by providing functions to query information from the user via the command line.
/// It is useful for deployment scripts or configuration wizards running in an interactive terminal.
///
/// Examples:
/// ```tolk
/// // Simple text input
/// val name = prompt("Enter your name:", "Guest");
///
/// // Confirmation
/// if (confirm("Do you want to deploy to mainnet?", false, "This will cost real TON.")) {
///     // perform deployment
/// } else {
///     println("Aborted.");
/// }
///
/// // Selection
/// val network = select("Choose network:", ["Mainnet", "Testnet", "Local"]);
/// ```

import "../ffi/ffi"

/// Prompts the user for input.
///
/// Placeholder is the text that will be displayed in the input field as
/// a hint to the user.
///
/// Example:
/// ```tolk
/// val name = prompt("What is your name?", "John");
/// println(format1("Hello {}", name));
/// ```
fun prompt(message: slice, placeholder: slice = ""): slice {
    return ffi.prompt(message, placeholder);
}

/// Prompts the user to select an option from a list of variants.
///
/// Example:
/// ```tolk
/// val name = select("What is your name?", ["John", "Jane", "Jim"] as tuple);
/// println(format1("Hello {}", name));
/// ```
fun select(message: slice, variants: tuple): slice {
    return ffi.select(message, variants);
}

/// Prompts the user to confirm an action.
///
/// Example:
/// ```tolk
/// val answer = confirm("Are you sure?", true, "Your data will be deleted");
/// if (answer) {
///     println("Data deleted");
/// }
/// ```
fun confirm(message: slice, default: bool, helpMessage: slice): bool {
    return ffi.confirm(message, default, helpMessage);
}

fun ffi.prompt(message: slice, placeholder: slice = ""): slice asm "205 EXTCALL"
fun ffi.select(message: slice, variants: tuple): slice asm "206 EXTCALL"
fun ffi.confirm(message: slice, default: bool, helpMessage: slice): bool asm "207 EXTCALL"
