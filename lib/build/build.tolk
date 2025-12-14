/// Module for building contracts.
///
/// This module provides the capability to compile Tolk contracts dynamically during the execution of tests or scripts.
/// It interfaces with the Tolk compiler to generate the code cell for a given contract.
import "../ffi/ffi"

/// Build a contract with a specified name and optional path.
/// 
/// If path is ommited, Acton will search for a contract with the given name in the Acton.toml file.
/// If no contract with the given name is found, test will fail.
/// 
/// Example:
/// 
/// ```tolk
/// val cell = build("jetton-wallet");
/// ```
/// 
/// If you want to build a contract from a specific path, provide the path as the second argument.
/// 
/// Example:
/// ```tolk
/// val cell = build("jetton-wallet", "contracts/jetton-wallet-contract.tolk");
/// ```
fun build(name: slice, path: slice = ""): cell asm "6 EXTCALL"
