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
@inline
fun vm.getConfigParam<T>(index: int): T {
    var c7 = vm.getC7();
    var params: tuple = c7.get(0);
    return params.get(index);
}

/// Generic helper to overwrite a single config parameter.
@inline
fun vm.setConfigParam<T>(value: T, index: int) {
    var c7 = vm.getC7();
    var params: tuple = c7.get(0);
    params.set(value, index);
    c7.set(params, 0);
    vm.setC7(c7);
}

/// Override `blockchain.now()` for deterministic testing.
@inline
fun vm.setTime(now: int) {
    vm.setConfigParam(now, 3);
}

/// Override the **block** logical time.
@inline
fun vm.setBlockLogicalTime(blockLogicalTime: int) {
    vm.setConfigParam(blockLogicalTime, 4);
}

/// Override the **transaction** logical time.
@inline
fun vm.setLogicalTime(logicalTime: int) {
    vm.setConfigParam(logicalTime, 5);
}

/// Override the original balance of the contract.
@inline
fun vm.setOriginalBalance(balance: coins, extraCurrencies: dict = null) {
    vm.setConfigParam([balance, extraCurrencies], 7);
}
