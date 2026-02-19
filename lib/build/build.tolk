/// Module for building contract code cells.
///
/// `build(name, path)` can compile a `.tolk` contract or load a prebuilt `.boc` file.
/// For `.tolk` sources, build results are cached in-memory for the current run and
/// persisted in `.acton/cache` for reuse across runs.

/// Build a contract with a specified name and optional path.
///
/// `name` is the contract id key in `Acton.toml` (`[contracts.<id>]`), not the display
/// name stored in `[contracts.<id>].name`.
///
/// If `path` is omitted (empty string), Acton looks up the contract id in `Acton.toml`.
/// If no contract with that id is found, execution fails.
///
/// Example:
///
/// ```tolk
/// val cell = build("jetton-wallet");
/// ```
///
/// If `path` is non-empty, `Acton.toml` lookup is skipped and that path is used directly.
/// The path can point to:
/// - a `.tolk` file (compiled, with in-memory and `.acton/cache` caching),
/// - a `.boc` file (loaded directly as code cell, without compilation).
///
/// Example:
/// ```tolk
/// val cell = build("jetton-wallet", "contracts/jetton-wallet-contract.tolk");
/// ```
///
/// Example with prebuilt code:
/// ```tolk
/// val cell = build("jetton-wallet", "build/jetton-wallet.boc");
/// ```
///
/// The path should be absolute or relative to the project root (the directory with `Acton.toml`).
fun build(name: string, path: string = ""): cell asm "6 EXTCALL"
