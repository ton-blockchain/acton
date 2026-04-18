/// Module for public API to be used in tests (which do NOT interact with real blockchain).
/// 
/// You cover contracts with tests to simulate their behavior in a real blockchain.
/// Use methods `testing.xxx` in combination with `net.xxx` in `some-file.test.tolk`.
/// 
/// Writing tests differs from writing scripts that operate testnet/mainnet.
/// In tests, you don't connect your wallet, you use `testing.treasury` (unlimited source of money).
/// In tests, you may set blockchain config, set any account at any address, etc.
/// In tests, you don't wait for network responses: all transactions are emulated.
/// 

import "_impl_emul"
import "../ffi"
import "config"
import "network"
import "../testing/assert"
import "../types/transaction"
import "../types/out_actions"

/// Root module for public API to be used in tests along with `net.xxx` methods.
struct testing

/// Read the TVM control register C7.
fun testing.getC7OutsideContract(): tuple
    asm "c7 PUSHCTR"

/// Write a new value to the TVM control register C7.
fun testing.setC7OutsideContract(c7: tuple): void
    asm "c7 POPCTR"

/// Generic helper to overwrite a single config parameter.
fun __acton_impl_setConfigParam<T>(value: T, index: int): void {
    var c7 = testing.getC7OutsideContract();
    var params = c7.get(0) as tuple;
    params.set(value, index);
    c7.set(params, 0);
    testing.setC7OutsideContract(c7);
}

/// Opaque cursor for step-by-step transaction execution.
///
/// Use [testing.createTraceIterationCursor] to create it, 
/// then drain it in parts with [TxCursor.executeN], [TxCursor.executeTill], or [TxCursor.executeAllRemaining].
struct TxCursor {
    id: int
}

/// Returns `true` when the cursor has no pending transactions to execute.
fun TxCursor.isDone(self): bool {
    return ffi.isMessageIterDone(self.id);
}

/// Discards any remaining pending transactions for this cursor.
fun TxCursor.close(self): void {
    ffi.closeMessageIter(self.id);
}

/// Executes up to [n] transactions from the cursor and returns only this executed segment.
///
/// This is useful when you need to inspect the first few hops of a long chain without
/// draining the whole queue.
fun TxCursor.executeN(self, n: int): SendResultList {
    return ffi.executeMessageIterN(self.id, n);
}

/// Executes transactions until one of them matches [params], then returns the executed segment.
///
/// The matching transaction is included in the returned list. If no matching transaction is found
/// before the queue is exhausted, this function fails the test.
fun TxCursor.executeTill<Msg = unknown>(self, params: SearchParams = {}): SendResultList {
    val declaredPrefix = Msg.__getDeclaredPackPrefix();
    if (declaredPrefix != -1) {
        params.opcode = declaredPrefix;
    }

    val txs = ffi.executeMessageIterTill(self.id, params.toPredicateFFITuple());
    if (txs == null) {
        Assert.fail("executeTill() did not find a matching transaction before queue exhaustion");
    }

    return txs;
}

/// Executes all remaining pending transactions and returns only this executed segment.
fun TxCursor.executeAllRemaining(self): SendResultList {
    return ffi.executeMessageIterFrom(self.id);
}

/// Start step-by-step execution for a message sent from [from].
///
/// In contrast to [net.send], this method does not execute the whole transaction chain eagerly.
/// Instead it returns a cursor that can be advanced in batches.
///
/// This API is emulation-only and is intended for programmable partial execution workflows
/// such as large cascades and interleaving multiple message chains.
fun testing.createTraceIterationCursor(from: address, message: OutMessage): TxCursor {
    return {
        id: ffi.startMessageIter(message.messageCell, from.toCell()),
    };
}

/// Send a single message from a specified address.
///
/// In contrast to [net.send], this method doesn't process out messages, but returns
/// a single send result with this out messages.
///
/// This method is useful to simulate a bounced message.
///
/// Example:
/// ```tolk
/// val bouncedMsg = createMessage({
///     bounce: false,
///     value: ton("0.95"),
///     dest: deployer.address,
///     body: beginCell().storeUint(0xFFFFFFFF, 32).storeSlice(body.toCell().beginParse()).endCell(),
/// }).bounced();
///
/// testing.processSingleTraceStep(notDeployer.address, bouncedMsg);
/// ```
///
/// See [net.send] if you need to send a message and process out messages.
fun testing.processSingleTraceStep(from: address, message: OutMessage): SendResult {
    val res = ffi.sendSingleMessage(message.messageCell, from.toCell())!;
    return SendResult.fromTuple(res);
}

/// Runs a tick or tock transaction on the specified account.
///
/// Tick-tock transactions have no incoming message and are triggered by the blockchain
/// on special accounts (e.g., elector, config).
///
/// Example:
/// ```tolk
/// val results = testing.runTickTock(electorAddress, false); // run tick
/// val results = testing.runTickTock(electorAddress, true);  // run tock
/// ```
fun testing.runTickTock(onAccount: address, isTock: bool): SendResultList {
    return ffi.runTickTock(onAccount, isTock);
}

/// Treasury is a special contract that is used to deploy other contracts.
///
/// To create a treasury, use the [testing.treasury] method.
struct Treasury {
    address: address
}

/// Create a treasury with a specified name.
///
/// Calling this method with same name will return the same treasury (the same address).
///
/// Example:
/// ```tolk
/// get fun `test mint`() {
///     val treasury = testing.treasury("treasury");
///     // ...
/// }
/// ```
///
/// Note: in scripts always use [scripts.wallet] instead.
fun testing.treasury(name: string): Treasury {
    if (net.isBroadcasting()) {
        Assert.fail("Cannot use treasury with `--net`, don't use `testing.treasury()` in scripts, use `scripts.wallet()` instead");
    }

    val stateInit: ContractState = {
        code: __acton_impl_treasuryCode(),
        data: beginCell().storeSlice(name.beginParse()).endCell(),
    };

    val deployer = address("0:0000000000000000000000000000000000000000000000000000000000000000");
    val address = AutoDeployAddress { stateInit }.calculateAddress();
    net.registerAddress(address, name);

    val deployMsg = createMessage({
        bounce: false,
        value: ton("10000"),
        dest: { workchain: 0, stateInit },
    });
    // Deploy treasury
    net.send(deployer, deployMsg);
    return Treasury { address };
}

fun __acton_impl_treasuryCode(): cell asm """
    "te6cckEBBAEARQABFP8A9KQT9LzyyAsBAgEgAwIAWvLT/+1E0NP/0RK68qL0BNH4AH+OFiGAEPR4b6UgmALTB9QwAfsAkTLiAbPmWwAE0jD+omUe" base64>B B>boc PUSHREF
"""

/// Returns current account state for address or null if no information available or
/// contract is not deployed.
///
/// The account state includes balance, storage information, and other account data.
/// See [TlbAccountInfo] type for more information.
///
/// Example:
/// ```tolk
/// val state = testing.getAccountState(contract.address);
/// if (state != null) {
///     println("Balance: {:ton}", state.storage.balance.grams);
/// } else {
///     println("Account not found");
/// }
/// ```
fun testing.getAccountState(addr: address): TlbAccountInfo? {
    return __acton_impl_getAccountState(addr)
}

/// Returns storage fee for address for specified amount of seconds.
///
/// Returns `null` if the contract with given address is not deployed.
///
/// Example:
/// ```tolk
/// val storageFee = testing.getAccountStorageFee(contract.address, 86400); // 1 day
/// if (storageFee != null) {
///     println("Daily storage fee: {:ton}", storageFee);
/// }
/// ```
fun testing.getAccountStorageFee(addr: address, seconds: int): coins? {
    return __acton_impl_getAccountStorageFee(addr, seconds);
}

/// Set the current Unix timestamp for time forwarding emulation.
///
/// This is useful for testing time-dependent contracts.
///
/// Example:
/// ```tolk
/// val futureTime = testing.getNow() + 1000; // 1000 seconds from now
/// testing.setNow(futureTime);
/// ```
fun testing.setNow(now: uint32): void {
    // Update C7[0][3] (unixtime) FIRST so blockchain.now() stays in sync
    __acton_impl_setConfigParam(now, 3);
    // Then update the WorldState time for tick-tock transactions
    ffi.setNow(now);
}

/// Get the current Unix timestamp used in emulation.
///
/// Returns the current time in the emulated blockchain.
///
/// Example:
/// ```tolk
/// val currentTime = testing.getNow();
/// println("Current emulation time: {}", currentTime);
/// ```
fun testing.getNow(): uint32 {
    return ffi.getNow();
}

/// Topup an address with a specified amount of TON.
///
/// Example:
/// ```tolk
/// testing.topUp(address, ton("10"));
/// ```
///
/// Note: This function for tests only!
/// For testnet TON airdrop onto your wallet, use `acton wallet airdrop` command.
fun testing.topUp(to: address, amount: coins): void {
    val treasury = testing.treasury("treasury");
    val topupMsg = createMessage({
        bounce: BounceMode.NoBounce,
        value: amount,
        dest: to,
    });
    net.send(treasury.address, topupMsg);
}

/// Get the balance of an address.
///
/// Returns 0 if the address is not deployed.
///
/// Example:
/// ```tolk
/// val balance = testing.getAccountBalance(address);
/// println("{:ton}", balance);
/// ```
fun testing.getAccountBalance(of: address): coins {
    return __acton_impl_getAccountBalance(of);
}

/// Returns the shard account state for an address.
///
/// The returned shard account can represent an uninitialized account.
///
/// Example:
/// ```tolk
/// val shard = testing.getShardAccount(addr);
/// if (shard != null) {
///     println("Last LT: {}", shard.lastTransLt);
/// }
/// ```
fun testing.getShardAccount(addr: address): TlbShardAccount? {
    return __acton_impl_getShardAccount(addr);
}

/// Sets the shard account state for an address in emulation.
///
/// Pass `null` to reset the account to an empty state.
///
/// Example:
/// ```tolk
/// val shard = testing.getShardAccount(addr);
/// if (shard != null) {
///     testing.setShardAccount(addr, shard);
/// }
/// ```
fun testing.setShardAccount(addr: address, account: TlbShardAccount?): void {
    if (account == null) {
        ffi.setShardAccount(addr, null);
        return;
    }
    ffi.setShardAccount(addr, account.toCell());
}

/// Load library by hash from emulated blockchain or real network and register it
/// in the current emulated blockchain.
///
/// Returns false if library is not found.
fun testing.loadAndRegisterLibrary(hash: string): bool {
    val cell = ffi.loadLibrary(hash);
    if (cell == null) {
        return false;
    }
    ffi.registerLibrary(cell);
    return true;
}

/// Register a library in the emulated blockchain.
fun testing.registerLibrary(libCell: cell): void {
    ffi.registerLibrary(libCell);
}

/// Save the current emulation world state to a JSON snapshot file.
///
/// Returns `true` on successful write and `false` on serialization or host I/O failure.
///
/// Example:
/// ```tolk
/// testing.topUp(addr, ton("3"));
/// val saved = testing.saveSnapshot("world-state.json");
/// expect(saved).toBeTrue();
/// ```
fun testing.saveSnapshot(pathToJsonFile: string): bool {
    return ffi.saveWorldStateSnapshot(pathToJsonFile);
}

/// Load an emulation world state from a JSON snapshot file.
///
/// On success, replaces the current local emulation state with the snapshot contents.
/// Returns `false` when the file is missing, malformed, or cannot be applied.
///
/// Example:
/// ```tolk
/// val loaded = testing.loadSnapshot("world-state.json");
/// expect(loaded).toBeTrue();
/// ```
fun testing.loadSnapshot(pathToJsonFile: string): bool {
    return ffi.loadWorldStateSnapshot(pathToJsonFile);
}

/// Sets the blockchain configuration.
///
/// This function updates the entire blockchain configuration with a new configuration dict.
/// The configuration is validated before being applied. If validation fails, the function
/// returns false and the configuration is not updated.
///
/// For detailed information about blockchain configuration parameters, see the
/// [TON Blockchain Configuration Reference](https://docs.ton.org/foundations/config).
///
/// Parameters:
/// - `config`: The new [BlockchainConfigMap] map containing configuration parameters (indices 0-90)
///
/// Returns: `true` if the configuration was successfully updated, `false` if validation failed.
///
/// Example:
/// ```tolk
/// var config = testing.getConfig();
/// val storagePrices = config.getStoragePrices();
/// var pricesDict = storagePrices;
/// pricesDict.setInitial(StoragePrices {
///     initialUnixTime: 0,
///     bitPrice: 1,
///     cellPrice: 1000,
///     masterchainBitPrice: 1,
///     masterchainCellPrice: 1000
/// });
/// config.setStoragePrices(pricesDict);
/// val success = testing.setConfig(config);
/// if (success) {
///     println("Configuration updated successfully");
/// }
/// ```
fun testing.setConfig(config: BlockchainConfigMap): bool {
    val root = config.toLowLevelDict();
    val result = ffi.setConfig(root);

    // the goal here is to update config in TWO places in c7: packed (original, root) and unpacked
    // (TVM automatically unpacks original c7 at start for optimization)
    if (result) {
        var c7 = testing.getC7OutsideContract();
        var params = c7.get(0) as tuple;
        var unpacked = params.get(14) as tuple;

        val storagePrices = config.getStoragePrices().getInitial();
        unpacked.set(storagePrices.toCell().beginParse(), 0);

        val masterchainGasPrices = config.getGasPrices(MASTERCHAIN);
        unpacked.set(masterchainGasPrices.toCell().beginParse(), 2);

        val basechainGasPrices = config.getGasPrices(BASECHAIN);
        unpacked.set(basechainGasPrices.toCell().beginParse(), 3);

        val masterchainFwd = config.getMsgForwardPrices(MASTERCHAIN);
        unpacked.set(masterchainFwd.toCell().beginParse(), 4);

        val basechainFwd = config.getMsgForwardPrices(BASECHAIN);
        unpacked.set(basechainFwd.toCell().beginParse(), 5);

        params.set(root, 9);        // 9: root index in c7
        params.set(unpacked, 14);   // 14: unpacked index in c7
        c7.set(params, 0);
        testing.setC7OutsideContract(c7);
    }

    return result;
}

/// Retrieves the current blockchain configuration.
///
/// Returns a map of all blockchain configuration parameters indexed from 0-90.
/// The configuration includes gas prices, storage costs, validator settings, and other
/// network parameters. Use specialized getter functions like [BlockchainConfigMap.getGasPrices],
/// [BlockchainConfigMap.getStoragePrices], or [BlockchainConfigMap.getGlobalVersion] 
/// to access specific parameters.
///
/// For detailed information about blockchain configuration parameters, see the
/// [TON Blockchain Configuration Reference](https://docs.ton.org/foundations/config).
///
/// Returns: A [BlockchainConfigMap] containing all current blockchain configuration parameters.
///
/// Example:
/// ```tolk
/// val config = testing.getConfig();
///
/// // Access specific configuration
/// val gasPrice = config.getGasPrices(BASECHAIN);
/// val storagePrices = config.getStoragePrices();
/// val globalVersion = config.getGlobalVersion();
///
/// println("Network version: {}", globalVersion.version);
/// ```
fun testing.getConfig(): BlockchainConfigMap {
    return createMapFromLowLevelDict(ffi.getConfig());
}

/// Check if a contract with a specified address is deployed.
///
/// Returns `true` if the contract is deployed, `false` otherwise.
///
/// Example:
/// ```tolk
/// fun JettonWallet.getJettonBalance(self): coins {
///     if (!testing.isDeployed(self.address)) {
///         return 0; // return 0 if contract is not deployed
///     }
///
///     // and if deployed, actually run get method
///     val result = net.runGetMethod<JettonWalletDataReply>(self.address, "get_wallet_data");
///     return result.jettonBalance;
/// }
/// ```
fun testing.isDeployed(addr: address): bool {
    return ffi.isDeployed(addr.toCell());
}

/// Returns out actions from the control register C5 at that moment.
///
/// Example:
/// ```tolk
/// val outActions = testing.outActions();
/// println(outActions.size());
/// ```
fun testing.outActions(): OutActionList {
    return __acton_impl_parseOutActions(__acton_impl_getC5());
}


fun ffi.sendSingleMessage(msg: cell, from: cell): tuple? asm "30 EXTCALL"
fun ffi.startMessageIter(msg: cell, from: cell): int asm "39 EXTCALL"
fun ffi.executeMessageIterN(cursorId: int, n: int): SendResultList asm "40 EXTCALL"
fun ffi.executeMessageIterTill(cursorId: int, params: tuple): SendResultList? asm "41 EXTCALL"
fun ffi.executeMessageIterFrom(cursorId: int): SendResultList asm "42 EXTCALL"
fun ffi.isMessageIterDone(cursorId: int): bool asm "43 EXTCALL"
fun ffi.closeMessageIter(cursorId: int): void asm "44 EXTCALL"
fun ffi.setNow(now: int): void asm "28 EXTCALL"
fun ffi.getNow(): int asm "29 EXTCALL"
fun ffi.setConfig(config: dict): bool asm "32 EXTCALL"
fun ffi.setShardAccount(addr: address, account: cell?): void asm "34 EXTCALL"
fun ffi.registerLibrary(cell: cell): void asm "18 EXTCALL"
fun ffi.saveWorldStateSnapshot(path: string): bool asm "37 EXTCALL"
fun ffi.loadWorldStateSnapshot(path: string): bool asm "38 EXTCALL"
fun ffi.runTickTock(onAccount: address, isTock: bool): SendResultList asm "36 EXTCALL"
