/// Module for user prompts.
///
/// This module enables interactive scripts by providing functions to query information from the user via the command line.
/// It is useful for deployment scripts or configuration wizards running in an interactive terminal.
///
/// Deterministic fallback matrix:
/// - `prompt(...)`: returns `default` in non-interactive mode
/// - `select(...)`: returns the `defaultIndex` option in non-interactive mode
/// - `confirm(...)`: returns `default` in non-interactive mode
/// - `promptInt(...)`: parses `default` in non-interactive mode
/// - `promptAddress(...)`: parses `default` in non-interactive mode
/// - `promptWallet(...)`: returns `"emulated-wallet"` in emulate mode; in
///   TON Connect mode it returns `"tonconnect"`; otherwise, in broadcast mode it
///   auto-selects the only configured wallet, but fails when no wallet is available
///   or wallet choice cannot be collected interactively
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

import "ffi"
import "fmt"

/// Prompts the user for input.
///
/// - `placeholder` is a grey hint shown inside the empty input field. It is not
///   returned as the result and disappears as soon as the user starts typing.
/// - `default` is the value returned when the user presses Enter without typing
///   anything. It is displayed next to the question (e.g. `(John)`).
///
/// Non-interactive behavior:
/// - Returns `default`; if `default` is omitted, returns `""`.
///
/// Example:
/// ```tolk
/// val name = prompt("What is your name?", "type your name", "John");
/// println("Hello {}", name);
/// ```
fun prompt(message: string, placeholder: string = "", default: string = ""): string {
    return ffi.prompt(message, placeholder, default);
}

/// Prompts the user to select an option from a list of variants.
///
/// Input and fallback behavior:
/// - `variants` is expected to be a tuple of strings.
/// - If `variants` is not a tuple, returns `""`.
/// - Non-string tuple items are silently ignored.
/// - `defaultIndex` positions the cursor on the given option when the prompt
///   opens, so the user can accept it by pressing Enter. Out-of-range values
///   are clamped to a valid index (defaults to `0`).
/// - In non-interactive mode returns the `defaultIndex` option.
/// - If interactive prompt execution fails, returns `""`.
///
/// Example:
/// ```tolk
/// val name = select("What is your name?", ["John", "Jane", "Jim"], 1);
/// println("Hello {}", name);
/// ```
fun select(message: string, variants: array<string>, defaultIndex: int = 0): string {
    return ffi.select(message, variants, defaultIndex);
}

/// Prompts the user to confirm an action.
///
/// Default and fallback behavior:
/// - `default` is forwarded to the interactive host confirm prompt.
/// - In non-interactive mode returns `default`.
/// - If interactive prompt execution fails, result is `default`.
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

/// Prompts the user for an integer.
///
/// Interactive behavior:
/// - Invalid input is rejected by the host prompt validator until the user
///   corrects it.
/// - Pressing Enter accepts `default` when it is non-empty.
///
/// Non-interactive behavior:
/// - Parses and returns `default`.
/// - If the selected value is empty or invalid, execution fails with the same
///   integer parser error as `parseInt(...)`.
fun promptInt(message: string, default: string = "", placeholder: string = ""): int {
    return ffi.promptInt(message, placeholder, default);
}

/// Prompts the user for a TON address.
///
/// Interactive behavior:
/// - Invalid input is rejected by the host prompt validator until the user
///   corrects it.
/// - Pressing Enter accepts `default` when it is not `null`.
///
/// Non-interactive behavior:
/// - Parses and returns `default`.
/// - If the selected value is empty or invalid, execution fails with the same
///   address parser error as `parseAddress(...)`.
fun promptAddress(
    message: string,
    default: address? = null,
    placeholder: string = "EQ... / kQ...",
): address {
    return ffi.promptAddress(
        message,
        placeholder,
        default == null ? "" : format("{}", default),
    );
}

/// Prompts the user to select a wallet from the configured wallets.
///
/// Behavior (broadcast mode, i.e. `--net` is set):
/// - With `--tonconnect`, returns `"tonconnect"`.
/// - If no wallets are configured, fails with an error suggesting how to create one.
/// - If only one wallet is configured, returns its name automatically.
/// - If multiple wallets are configured, shows an interactive selection.
/// - In non-interactive mode (e.g. CI), fails with an error asking to specify the wallet explicitly.
/// - If the interactive wallet picker is opened but cancelled or otherwise fails,
///   `promptWallet()` throws.
///
/// In emulate mode (no `--net`), returns the stable placeholder `"emulated-wallet"`:
/// `scripts.wallet(name)` accepts any name in emulate mode, so there is nothing real to pick.
///
/// Example:
/// ```tolk
/// val walletName = promptWallet("Select a wallet:");
/// ```
fun promptWallet(message: string): string {
    val name = ffi.promptWallet(message);
    if (name == null) {
        throw 1;
    }
    return name;
}

fun ffi.prompt(message: string, placeholder: string, default: string): string
    asm "205 EXTCALL"

fun ffi.select(message: string, variants: array<string>, defaultIndex: int): string
    asm "206 EXTCALL"

fun ffi.confirm(message: string, default: bool, helpMessage: string): bool
    asm "207 EXTCALL"

fun ffi.promptWallet(message: string): string?
    asm "208 EXTCALL"

fun ffi.promptInt(message: string, placeholder: string, default: string): int
    asm "209 EXTCALL"

fun ffi.promptAddress(message: string, placeholder: string, default: string): address
    asm "210 EXTCALL"
