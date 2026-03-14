/// Module for VM interaction.
///
/// This module provides low-level access to the TON Virtual Machine (TVM) state during test execution.
/// It allows reading and writing to control registers (C5, C7) and configuring execution parameters.
///
/// Examples:
/// ```tolk
/// // Mock time
/// vm.setTime(1700000000);
///
/// // Register a mock library
/// vm.registerLibrary(myLibCell);
///
/// // Inspect actions manually
/// val rawActions = vm.getC5();
/// ```
import "../ffi/ffi"

/// Holds the methods to change the VM state.
struct vm

/// Read control register C7.
fun vm.getC7(): tuple
    asm "c7 PUSHCTR"

/// Write a new value to control register C7.
fun vm.setC7(c7: tuple): void
    asm "c7 POPCTR"

/// Read control register C5.
fun vm.getC5(): cell
    asm "c5 PUSHCTR"

/// Write a new value to control register C5.
fun vm.setC5(cell: cell): void
    asm "c5 POPCTR"

/// Generic helper to get a single config parameter.
fun vm.getConfigParam<T>(index: int): T {
    val c7 = vm.getC7();
    val params = c7.get(0) as tuple;
    return params.get(index) as T;
}

/// Generic helper to overwrite a single config parameter.
fun vm.setConfigParam<T>(value: T, index: int): void {
    var c7 = vm.getC7();
    var params = c7.get(0) as tuple;
    params.set(value, index);
    c7.set(params, 0);
    vm.setC7(c7);
}

const CONFIG_ROOT_INDEX = 9

/// Set the blockchain configuration root dictionary.
///
/// This function allows you to override the configuration root dictionary (parameter 9) in the VM state.
/// The config root is a dictionary containing blockchain configuration parameters.
/// It writes C7 slot 9 only.
///
/// Note that this does not recompute unpacked config in C7 slot 14.
/// After [vm.setConfigRootDict], [vm.getConfigUnpacked] can still return stale values.
fun vm.setConfigRootDict(root: dict): void {
    var c7 = vm.getC7();
    var params = c7.get(0) as tuple;

    params.set(root, CONFIG_ROOT_INDEX);
    c7.set(params, 0);
    vm.setC7(c7);
}

const CONFIG_UNPACKED_INDEX = 14

/// Set the unpacked configuration parameters tuple.
///
/// This function allows you to override the unpacked configuration parameters (parameter 14) in the VM state.
/// The unpacked config contains a tuple of configuration values extracted from the config dictionary.
/// It writes C7 slot 14 directly and does not validate tuple shape.
///
/// Use [vm.getConfigUnpacked] as a template for expected tuple layout.
/// If this tuple is malformed (for example, too short), later operations that assume
/// specific indices can fail with runtime errors such as `tuple index out of range`.
fun vm.setConfigUnpacked(values: tuple): void {
    var c7 = vm.getC7();
    var params = c7.get(0) as tuple;

    params.set(values, CONFIG_UNPACKED_INDEX);
    c7.set(params, 0);
    vm.setC7(c7);
}

/// Get the unpacked configuration parameters tuple.
///
/// Retrieves the unpacked configuration parameters (parameter 14) from the current VM state.
/// Returns a tuple of configuration values from C7 slot 14 as-is.
/// This value is independent from C7 slot 9 set via [vm.setConfigRootDict].
///
/// Example:
/// ```tolk
/// val unpacked = vm.getConfigUnpacked();
/// ```
fun vm.getConfigUnpacked(): tuple {
    val c7 = vm.getC7();
    val params = c7.get(0) as tuple;

    return params.get(CONFIG_UNPACKED_INDEX) as tuple;
}

/// Override `blockchain.now()` for deterministic testing.
///
/// Writes C7 parameter tuple slot 3 and affects `blockchain.now()` reads.
fun vm.setTime(now: int): void {
    vm.setConfigParam(now, 3);
}

/// Override the **block** logical time.
///
/// Writes C7 parameter tuple slot 4.
/// Does not change slots 3 (`now`) or 5 (`logical time`).
fun vm.setBlockLogicalTime(blockLogicalTime: int): void {
    vm.setConfigParam(blockLogicalTime, 4);
}

/// Override the **transaction** logical time.
///
/// Writes C7 parameter tuple slot 5.
/// Does not change slots 3 (`now`) or 4 (`block logical time`).
fun vm.setLogicalTime(logicalTime: int): void {
    vm.setConfigParam(logicalTime, 5);
}

/// Override the original balance of the contract.
///
/// Writes C7 parameter tuple slot 7 as `[balance, extraCurrencies]`.
fun vm.setOriginalBalance(balance: coins, extraCurrencies: dict = null): void {
    vm.setConfigParam(tuple [balance, extraCurrencies], 7);
}

/// Register a new library for current test.
///
/// Re-registering the same library cell is idempotent and does not change C5/C7.
fun vm.registerLibrary(cell: cell): void {
    ffi.registerLibrary(cell);
}

/// Convert a string representation of address to an internal [address] value.
///
/// Accepted forms include raw addresses (`0:...`) and user-friendly TON addresses
/// (base64/base64url with flags).
/// The returned value is normalized as an internal address; rendered formatting may
/// differ from input flags, while representing the same account.
/// Invalid input fails with an error like `Failed to convert address from <input>`.
///
/// Example:
/// ```tolk
/// val address = vm.convertAddress("kQCPFJYFqhowz0BRAfqNPdLOC1_Rlupki_0BezWU6rHk5lbt");
/// ```
fun vm.convertAddress(addr: string): address {
    val cell = ffi.convertAddress(addr);
    return cell.beginParse() as address;
}

/// Decode a root cell from hex-encoded BOC bytes.
///
/// `cellHex` must be a valid BOC serialized as hex.
/// Invalid or malformed input (including non-BOC payloads) fails with an error like
/// `Failed to decode cell hex <input>`.
fun vm.cellFromHex(cellHex: string): cell {
    return ffi.cellFromHex(cellHex);
}

fun ffi.registerLibrary(cell: cell): void
    asm "18 EXTCALL"

fun ffi.convertAddress(addr: string): cell
    asm "19 EXTCALL"

fun ffi.cellFromHex(cellHex: string): cell
    asm "20 EXTCALL"
