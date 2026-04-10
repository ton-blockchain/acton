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
/// Non-interactive fallback:
/// - Returns `""` when user input cannot be collected (for example, non-interactive runner).
///
/// Example:
/// ```tolk
/// val name = prompt("What is your name?", "John");
/// println(format1("Hello {}", name));
/// ```
fun prompt(message: string, placeholder: string = ""): string {
    return ffi.prompt(message, placeholder);
}

/// Prompts the user to select an option from a list of variants.
///
/// Input and fallback behavior:
/// - `variants` is expected to be a tuple of strings.
/// - If `variants` is not a tuple, returns `""`.
/// - Non-string tuple items are silently ignored.
/// - If prompt execution fails (including non-interactive mode), returns `""`.
///
/// Example:
/// ```tolk
/// val name = select("What is your name?", ["John", "Jane", "Jim"] as tuple);
/// println(format1("Hello {}", name));
/// ```
fun select(message: string, variants: tuple): string {
    return ffi.select(message, variants);
}

/// Prompts the user to confirm an action.
///
/// Default and fallback behavior:
/// - `default` is forwarded to the interactive host confirm prompt.
/// - If prompt execution fails, result is `false` even when `default` is `true`
///   (this includes non-interactive mode and known empty-input failure flows).
///
/// Example:
/// ```tolk
/// val answer = confirm("Are you sure?", true, "Your data will be deleted");
/// if (answer) {
///     println("Data deleted");
/// }
/// ```
fun confirm(message: string, default: bool, helpMessage: string): bool {
    return ffi.confirm(message, default, helpMessage);
}

/// Prompts the user to select a wallet from the configured wallets.
///
/// Behavior:
/// - If no wallets are configured, fails with an error suggesting how to create one.
/// - If only one wallet is configured, returns its name automatically.
/// - If multiple wallets are configured, shows an interactive selection.
/// - In non-interactive mode (e.g. CI), fails with an error asking to specify the wallet explicitly.
///
/// Example:
/// ```tolk
/// val walletName = promptWallet("Select a wallet:");
/// ```
fun promptWallet(message: string): string {
    return ffi.promptWallet(message);
}

fun ffi.prompt(message: string, placeholder: string = ""): string asm "205 EXTCALL"
fun ffi.select(message: string, variants: tuple): string asm "206 EXTCALL"
fun ffi.confirm(message: string, default: bool, helpMessage: string): bool asm "207 EXTCALL"
fun ffi.promptWallet(message: string): string asm "208 EXTCALL"
