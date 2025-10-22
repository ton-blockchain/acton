/// Build a contract from a specified path.
/// 
/// Example:
/// ```
/// val cell = build("jetton-wallet", "contracts/jetton-wallet-contract.tolk");
/// ```
fun build(name: slice, path: slice): cell asm "6 EXTCALL"
