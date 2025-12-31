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
    asm "c7 PUSHCTR";

/// Write a new value to control register C7.
fun vm.setC7(c7: tuple): void
    asm "c7 POPCTR";

/// Read control register C5.
fun vm.getC5(): cell
    asm "c5 PUSHCTR";

/// Write a new value to control register C5.
fun vm.setC5(cell: cell): void
    asm "c5 POPCTR";

/// Generic helper to get a single config parameter.
fun vm.getConfigParam<T>(index: int): T {
    var c7 = vm.getC7();
    var params: tuple = c7.get(0);
    return params.get(index);
}

/// Generic helper to overwrite a single config parameter.
fun vm.setConfigParam<T>(value: T, index: int) {
    var c7 = vm.getC7();
    var params: tuple = c7.get(0);
    params.set(value, index);
    c7.set(params, 0);
    vm.setC7(c7);
}

/// Override `blockchain.now()` for deterministic testing.
fun vm.setTime(now: int) {
    vm.setConfigParam(now, 3);
}

/// Override the **block** logical time.
fun vm.setBlockLogicalTime(blockLogicalTime: int) {
    vm.setConfigParam(blockLogicalTime, 4);
}

/// Override the **transaction** logical time.
fun vm.setLogicalTime(logicalTime: int) {
    vm.setConfigParam(logicalTime, 5);
}

/// Override the original balance of the contract.
fun vm.setOriginalBalance(balance: coins, extraCurrencies: dict = null) {
    vm.setConfigParam([balance, extraCurrencies], 7);
}

/// Register a new library for current test.
fun vm.registerLibrary(cell: cell) {
    ffi.register_library(cell)
}

/// Convert a slice with string representation of address to an address type.
///
/// Example:
/// ```tolk
/// val address = net.convertAddress("kQCPFJYFqhowz0BRAfqNPdLOC1_Rlupki_0BezWU6rHk5lbt");
/// ```
fun vm.convertAddress(addr: slice): address {
    val cell = ffi.convertAddress(addr);
    return cell.beginParse() as address
}

fun vm.cellFromHex(cell_hex: slice): cell {
    return ffi.cellFromHex(cell_hex)
}

fun ffi.register_library(cell: cell): void asm "18 EXTCALL"
fun ffi.convertAddress(addr: slice): cell asm "19 EXTCALL"
fun ffi.cellFromHex(cell_hex: slice): cell asm "20 EXTCALL"
