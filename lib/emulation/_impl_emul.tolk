/// Module with common implementations between `testing.xxx` and `scripts.xxx`.
/// 
/// Purpose: we want to expose namespaces `testing` and `scripts` for writing tests / scripts.
/// They have common functionality, but:
/// - in tests, it's always emulated
/// - in scripts, it's either emulated of operates on a real blockchain (with `--net` flag)
/// 
/// For example: `testing.getAccountState` and `scripts.fetchAccountState` have absolutely
/// identical implementation, but are intentionally named differently.
/// 
/// This "identical implementation" for such functions are in this module.
/// 

import "@stdlib/gas-payments"
import "../types/transaction"
import "../ffi"

fun __acton_impl_getAccountState(addr: address): TlbAccountInfo? {
    val cell = ffi.getAccountState(addr.toCell());
    if (cell == null) {
        return null;
    }
    val typedCell = cell as Cell<TlbAccountInfo>;
    return typedCell.load();
}

fun __acton_impl_getAccountStorageFee(addr: address, seconds: int): coins? {
    val state = __acton_impl_getAccountState(addr);
    if (state == null) {
        return null;
    }
    val usedStorage = state.storageStat.used;
    return calculateStorageFee(BASECHAIN, seconds, usedStorage.bits, usedStorage.cells);
}

fun __acton_impl_getAccountBalance(of: address): coins {
    val state = __acton_impl_getAccountState(of);
    if (state == null) {
        return 0;
    }
    return state.storage.balance.grams;
}

fun __acton_impl_getShardAccount(addr: address): TlbShardAccount? {
    val rawRes = ffi.getShardAccount(addr);
    if (rawRes == null) {
        return null;
    }
    return TlbShardAccount.fromCell(rawRes);
}


fun ffi.getAccountState(addr: cell): cell? asm "17 EXTCALL"
fun ffi.getShardAccount(addr: address): cell? asm "33 EXTCALL";
