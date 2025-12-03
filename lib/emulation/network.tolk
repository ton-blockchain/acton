import "@stdlib/common"
import "@stdlib/gas-payments"
import "../build/build"
import "../ffi/ffi"
import "../fmt"
import "../testing/assert"
import "../tlb/either"
import "../tlb/maybe"
import "../types/message"
import "../types/out_actions"
import "../types/transaction"
import "../vm/vm"

const CANNOT_RUN_GET_METHOD_OD_UNDEPLOYED_CONTRACT = 678
const CANNOT_RUN_GET_METHOD_OF_CONTRACT_WITHOUT_CODE = 679

/// Abstraction representing a blockchain and methods of interacting with it.
struct net

/// Represents a set of parameters for creating a message.
struct SendParams {
    /// Value to send with the message.
    value: coins
}

/// Information about external outgoing message.
struct ExtOutMsgInfo {
    /// Internal source address.
    src: address
    /// Optional external destination address.
    dst: address
    /// Logical time when the message was created.
    created_lt: int64
    /// Unix timestamp when the message was created.
    created_at: uint32
}

/// Represents a list of child transactions IDs (LT) for single send result.
type ChildTxsIds = tuple

/// Represents a list of out messages for single send result.
type OutMessages = tuple

/// Represents a list of external outgoing messages for single send result.
type ExtOutList = tuple

/// Returns the external outgoing message at the given index with given type.
///
/// Note that if message is not a given type, loading will fail with exit code 63.
/// If you want to handle this case, use [ExtOutList.atOrNull] instead.
fun ExtOutList.at<Msg>(self, index: int): ExtMessageRelaxed<Msg> {
    val cell = self.get<cell>(index) as Cell<ExtMessageRelaxed<Msg>>;
    return cell.load();
}

/// Returns the external outgoing message at the given index with given type or
/// null if message is not a given type.
///
/// Note that if message is not a given type, this method will return null instead
/// of throwing an exception. Use [ExtOutList.at] if you don't want to handle null case.
fun ExtOutList.atOrNull<Msg>(self, index: int): ExtMessageRelaxed<Msg>? {
    val cell = self.get<cell>(index) as Cell<ExtMessageRelaxed<Msg>>;
    try {
        return cell.load();
    } catch (e) {
        if (e == 63) {
            // opcode mismatch
            return null;
        }

        // other errors are not expected
        throw e;
    }
}

/// Returns the out message at the given index as a raw cell.
///
/// To get parsed message, use [OutMessages.at] method.
fun OutMessages.atCell(self, index: int): cell {
    return self.get<cell>(index);
}

/// Returns the out message at the given index as a parsed message of given type.
///
/// To get raw cell, use [OutMessages.atCell] method.
///
/// If actual message body has a different type, loading will fail.
fun OutMessages.at<Msg>(self, index: int): MessageRelaxed<Msg> {
    val cell = self.get<cell>(index) as Cell<MessageRelaxed<Msg>>;
    try {
        return cell.load();
    } catch (e) {
        if (e == 63) {
            // prefix mismatch
            val cell_generic = self.get<cell>(index) as Cell<MessageRelaxedGeneric>;
            val msg = cell_generic.load();

            val opcode = msg.loadOpcode();
            if (opcode == null) {
                throw e;
            }

            val opcode_type_name = ffi.type_name_by_opcode(opcode);

            Assert.fail(format4(
                "Out message is not a `{}` (opcode: 0x{:x}), but `{}` (opcode: 0x{:x})",
                Msg.typeName(),
                Msg.getDeclaredPackPrefix(),
                opcode_type_name == null ? "" : opcode_type_name,
                opcode
            ));
        }

        throw e;
    }
}

/// Represents a single send result obtained from [net.send] method.
struct SendResult {
    /// Raw cell with encoded transaction.
    tx: Cell<Transaction>
    /// List of child transactions IDs.
    /// This transactions are directly produced by this transaction.
    child_txs: ChildTxsIds
    /// Parent transaction LT that produced this transaction or null if this transaction is the root one.
    parent_lt: int?
    /// Raw cell with encoded out actions.
    /// Use [SendResult.allOutActions] to load all actions.
    out_actions: Cell<OutList>
    /// All out messages produced by this transaction.
    out_messages: OutMessages
    /// Amount of gas used by this transaction.
    gas_used: int
    /// All external outgoing messages produced by this transaction.
    externals: ExtOutList
}

/// Returns all out actions produced by this transaction as a list.
fun SendResult.allOutActions(self): OutActionList {
    return vm.parseOutActions(self.out_actions);
}

/// Represents a list of send results obtained from [net.send] method.
///
/// This list represent all transactions produced by a [net.send] call.
///
/// To get a single send result, use [SendResultList.at] method.
type SendResultList = tuple

/// Returns the send result at the given index.
fun SendResultList.at(self, index: int): SendResult {
    val tuple = self.get<tuple>(index);
    return SendResult.fromTuple(tuple);
}

/// Wait for the transaction to be applied on the blockchain.
///
/// This method is useful when broadcasting transactions to the real network.
/// It polls the network until the transaction is applied or the maximum number of attempts is reached.
///
/// In emulation mode (without `--broadcast`), this method returns immediately with `true`.
///
/// Returns `true` if transaction is found and `false` otherwise.
///
/// Example:
/// ```
/// val result = net.send(wallet.address, message, SEND_MODE_PAY_FEES_SEPARATELY);
/// val success = result.wait();
/// if (success) {
///     println("Transaction applied successfully");
/// }
/// ```
///
/// Example with custom parameters:
/// ```
/// val result = net.send(wallet.address, message, SEND_MODE_PAY_FEES_SEPARATELY);
/// val success = result.wait(true, 30, 3000); // quiet mode, 30 attempts, 3 seconds delay
/// ```
fun SendResultList.wait(
    self,
    quiet: bool = false,
    attempts: int = 20,
    sleepDuration: int = 2000,
): bool {
    if (!net.isBroadcasting()) {
        // in emulation mode we don't need to wait for anything
        return true
    }

    if (self.size() == 0) {
        // unexpected
        return false
    }

    val res = self.at(0);
    val tx = res.tx.load();
    val in_msg_cell = tx.messages.load().in_msg.unwrap();
    val in_msg = in_msg_cell.load();
    var body = in_msg.body;
    body.skipBits(1); // skip Either bit
    val body_hash = body.hash();
    val body_hash_slice = beginCell().storeUint(body_hash, 256).toSlice();
    return net.waitForTransaction(in_msg.info.dest, body_hash_slice, quiet, attempts, sleepDuration)
}

/// Represents a set of parameters to search for a transaction in the transaction list.
struct SearchParams {
    /// Destination address of the transaction.
    to: address? = null
    /// Source address of the transaction.
    from: address? = null
    /// Exit code of the transaction.
    exit_code: int32? = null
    /// Whether the transaction is a deploy transaction.
    ///
    /// Deploy transaction is a transaction that changes the `from` account status from "NonExisting" to "Active".
    deploy: bool? = null
    /// Whether the transaction is a bounced transaction.
    bounced: bool? = null
    /// Opcode of the in message.
    opcode: uint32? = null
    /// Action exit code of the transaction.
    action_exit_code: int32? = null
    /// Whether the compute phase was skipped.
    compute_phase_skipped: bool? = null
    /// Body of the in message (compared by hash).
    body: cell? = null
}

struct ExternalOutSearchParams {
    /// Source address of the external-out message.
    from: address? = null
    /// Destination address of the external-out message.
    to: any_address? = null
}

fun Message<RemainingBitsAndRefs, ExternalOutMessageInfo>.matches<TBody = never>(self, params: ExternalOutSearchParams = {}): bool {
    if (params.from != null && self.info.src != params.from) {
        return false;
    }

    if (params.to != null && self.info.dest != params.to) {
        return false;
    }

    if (TBody.getDeclaredPackPrefix() != null) {
        val body = self.loadBody();
        if (body.remainingBitsCount() < TBody.getDeclaredPackPrefixLen()) {
            return false;
        }

        val prefix = body.preloadUint(TBody.getDeclaredPackPrefixLen());
        if (prefix != TBody.getDeclaredPackPrefix()) {
            return false;
        }
    }

    return true;
}

/// Find a external-out message in the send result list by specified parameters.
///
/// Example:
/// ```
/// val tx = net.send(deployer.address, message, SEND_MODE_PAY_FEES_SEPARATELY);
/// val result = tx.findExternalOutMessage<IncrementEvent>({ from: counter.address });
/// expect(result).toBeDefined();
/// val tx = result.unwrap();
/// ```
fun SendResultList.findExternalOutMessage<TBody = never>(self, params: ExternalOutSearchParams = {}): Maybe<Message<TBody, ExternalOutMessageInfo>> {
    var transactionIndex = 0;
    while (transactionIndex < self.size()) {
        val externals = self.at(transactionIndex).externals;

        var externalIndex = 0;
        while (externalIndex < externals.size()) {
            val externalOut = externals
                .get<Cell<Message<RemainingBitsAndRefs, ExternalOutMessageInfo>>>(externalIndex)
                .load();

            if (externalOut.matches<TBody>(params)) {
                return Maybe<Message<TBody, ExternalOutMessageInfo>>.some(
                    Message{
                        info: externalOut.info,
                        init: externalOut.init,
                        body: EitherLeft{
                            value: TBody.fromSlice(externalOut.loadBody()),
                        },
                    },
                );
            }

            externalIndex += 1;
        }

        transactionIndex += 1;
    }

    return Maybe<Message<TBody, ExternalOutMessageInfo>>.none();
}

/// Find a transaction in the send result list by specified parameters.
///
/// Example:
/// ```
/// val tx = net.send(deployer.address, message, SEND_MODE_PAY_FEES_SEPARATELY);
/// val result = tx.findTransaction({ to: counter.address });
/// expect(result).toBeDefined();
/// val tx = result.unwrap();
/// ```
fun SendResultList.findTransaction<Msg = never>(self, params: SearchParams = {}): Maybe<Transaction> {
    params.opcode = Msg.getDeclaredPackPrefix();
    val tx = ffi.findTransactionByParams(self, params.toTuple());
    if (tx == null) {
        return Maybe<Transaction>.none();
    }
    val tx_cell = tx as Cell<Transaction>;
    return Maybe<Transaction>.some(tx_cell.load());
}

/// Internal method, don't use it directly.
fun net.sendFromNull(message: OutMessage, mode: int): TransactionList {
    return ffi.sendMessage(message.messageCell, mode)!;
}

/// Send a message from a specified address with specified mode.
///
/// Example:
/// ```
/// get fun `test-send-message`() {
///     // ...
///     val message = createMessage({
///         bounce: false,
///         dest: counter.address,
///         value: ton("1"),
///     });
///     net.send(deployer.address, message, SEND_MODE_PAY_FEES_SEPARATELY);
/// }
/// ```
///
/// Returns a list of send results representing all transactions produced by this send call.
fun net.send(from: address, message: OutMessage, mode: int): SendResultList {
    return ffi.sendMessageFrom(message.messageCell, from.toCell(), mode)!;
}

/// Treasury is a special contract that is used to deploy other contracts.
///
/// To create a treasury, use the `Network.treasury()` method.
struct Treasury {
    address: address
}

/// Create a treasury with a specified name.
///
/// Calling this method with same name will return the same treasury.
///
/// Example:
/// ```
/// get fun `test-mint`() {
///     val net = accuireNet("net");
///     val treasury = net.treasury("treasury");
///     // ...
/// }
/// ```
fun net.treasury(name: slice): Treasury {
    val state_init: ContractState = {
        code: __treasuryCode(),
        data: beginCell().storeSlice(name).endCell(),
    };

    val address = AutoDeployAddress { stateInit: state_init }.calculateAddress();
    net.registerAddress(address, name);

    val msg = createMessage({
        bounce: false,
        value: ton("10000"),
        dest: { workchain: 0, stateInit: state_init },
    });
    // Deploy treasury
    net.sendFromNull(msg, SEND_MODE_PAY_FEES_SEPARATELY);
    return Treasury { address };
}

struct Wallet {
    address: address
}

/// Creates a new wallet for use in scripts. Unlike `net.treasury`, the wallet
/// can refer to a real wallet described in Acton.toml, and therefore can be used
/// in scripts with `--broadcast` to execute transactions on the real blockchain.
///
/// Without `--broadcast`, the wallet refers to an internal wallet, created in the
/// same way as in `net.treasury`.
///
/// Note: in tests, use `net.treasury`.
fun net.wallet(name: slice): Wallet {
    if (ffi.isBroadcasting()) {
        val address_cell = ffi.getWalletByName(name);
        if (address_cell == null) {
            Assert.fail(format1("Cannot find wallet with name '{}', check Acton.toml for available wallets", name));
            throw 1;
        }
        val address = address_cell.beginParse().loadAddress();
        return Wallet { address }
    }

    return Wallet {
        address: net.treasury(name).address,
    }
}

/// Check if the script is running in broadcasting mode.
///
/// Returns `true` if the script was run with `--broadcast` flag, `false` otherwise.
///
/// This is useful for writing scripts that behave differently in emulation and broadcasting modes.
///
/// Example:
/// ```
/// if (net.isBroadcasting()) {
///     println("Running on real network");
/// } else {
///     println("Running in emulation mode");
/// }
/// ```
fun net.isBroadcasting(): bool {
    return ffi.isBroadcasting()
}

/// Enable broadcasting mode.
///
/// This function allows scripts to manually enable broadcasting behavior.
/// Useful for testing broadcasting logic in emulation mode.
///
/// Example:
/// ```
/// // Force broadcasting behavior even in emulation mode
/// net.enableBroadcast();
/// if (net.isBroadcasting()) {
///     println("Broadcasting mode enabled");
/// }
/// ```
fun net.enableBroadcast(): void {
    ffi.enableBroadcast();
}

/// Disable broadcasting mode.
///
/// This function allows scripts to manually disable broadcasting behavior.
///
/// Example:
/// ```
/// // Disable broadcasting behavior
/// net.disableBroadcast();
/// if (!net.isBroadcasting()) {
///     println("Broadcasting mode disabled");
/// }
/// ```
fun net.disableBroadcast(): void {
    ffi.disableBroadcast();
}

/// Run a get method of a contract by its name.
///
/// Imagine you have the following get method in the contract:
/// ```
/// get fun get_balance(): coins { ... }
/// ```
///
/// To call it, you can use the following code:
/// ```
/// val balance = net.runGetMethod<coins>(wallet.address, "get_balance");
/// ```
/// Since this get method doesn't accept any arguments, third argument can be omitted.
///
/// ## With argument
///
/// If the get method accepts arguments, you can pass them as a third argument.
/// ```
/// get fun get_wallet_address(ownerAddress: address): address { ... }
/// ```
/// ```
/// val address = net.runGetMethod<address>(wallet.address, "get_wallet_address", deployer.address);
/// ```
///
/// ## With struct return type
///
/// If get method returns a struct, specify the type of the struct as a first type argument:
/// ```
/// get fun get_jetton_data(): JettonDataReply { ... }
/// ```
/// ```
/// val data = net.runGetMethod<JettonDataReply>(wallet.address, "get_jetton_data");
/// ```
/// And provide glue method:
/// ```
/// fun JettonDataReply.fromTuple(t: tuple): JettonDataReply asm "<count fields> UNTUPLE"
/// ```
///
/// ## With tensor return type
///
/// If get method returns a tensor, specify tuple as a first type argument:
/// ```
/// get fun get_all_data(): (int, slice) { ... }
/// ```
/// ```
/// val data: tuple = net.runGetMethod(wallet.address, "get_all_data");
/// val first = data.get<int>(0);
/// val second = data.get<int>(1);
/// ```
///
/// If you need to run a get method by its ID, use [Network.runGetMethodById] instead.
fun net.runGetMethod<Ret, Args = tuple>(addr: address, name: slice, args: Args? = null): Ret {
    val method_id = (crc16(name) & 0xffff) | 0x10000;
    return impl.runGetMethodById(addr, method_id, name, args)
}

/// Run a get method of a contract by its ID.
///
/// If you need to run a get method by its name, use [Network.runGetMethod] instead.
///
/// Example:
/// ```
/// val data = net.runGetMethodById<JettonDataReply>(wallet.address, 0x17b02);
/// ```
///
/// See [Network.runGetMethod] for more information.
@noinline
fun net.runGetMethodById<Ret, Args = tuple>(addr: address, id: int, args: Args? = null): Ret {
    return impl.runGetMethodById(addr, id, "", args)
}

/// Check if a contract with a specified address is deployed.
fun net.isDeployed(addr: address): bool {
    return ffi.isDeployed(addr.toCell());
}

/// Get the code of a deployed contract by its address.
///
/// If the contract is not deployed, returns null.
fun net.getDeployedCode(addr: address): cell? {
    return ffi.getDeployedCode(addr.toCell());
}

/// Wait for a transaction with the specified message hash.
///
/// This method polls the network until the transaction is applied or the
/// maximum number of attempts is reached.
///
/// Returns true if transaction is found and false otherwise.
///
/// Example:
/// ```
/// val messageHash = "abc123...";
/// net.waitForTransaction(wallet.address, messageHash, 20, 2000);
/// ```
///
/// See [SendResultList.wait] for high-level API.
fun net.waitForTransaction(
    addr: address,
    messageBodyHash: slice,
    quiet: bool = false,
    attempts: int = 20,
    sleepDuration: int = 2000,
): bool {
    return ffi.waitForTransaction(
        addr.toCell(),
        messageBodyHash.toCell(),
        quiet,
        attempts,
        sleepDuration
    );
}

/// Generate a random address.
///
/// If a name is provided, the address will be registered with the given name.
/// This allows test runner to show the name instead of the address in the test output.
///
/// Example:
/// ```
/// val address = net.randomAddress("random_user");
/// ```
fun net.randomAddress(name: slice? = null): address {
    var builder = beginCell();
    builder.storeUint(0b10, 2);
    builder.storeUint(0b0, 1);
    builder.storeUint(0b0, 8);

    var i = 0;
    while (i < 32) {
        val num = random.range(256);
        builder.storeUint(num, 8);
        i += 1;
    }

    val address = address.fromValidBuilder(builder);
    if (name != null) {
        net.registerAddress(address, name);
    }
    return address;
}

/// Register an address with a name.
///
/// This allows test runner to show the name instead of the address in the test output.
///
/// For example, if your contract is not deployed yet, without calling this method,
/// the test runner will show raw address in the test output. In this case, you
/// likely want to call this method to register the address with a name.
///
/// Example:
/// ```
/// net.registerAddress(address, "my_address");
/// ```
fun net.registerAddress(addr: address, name: slice) {
    ffi.registerAddress(addr.toCell(), name);
}

/// Register a code cell with a name.
///
/// This allows test runner to show the name instead of the address in the test output
/// if the actual code of the contract is match to previously registered code.
///
/// Example:
/// ```
/// net.registerCodeCell(code, "my_code");
/// ```
fun net.registerCodeCell(code: cell, name: slice) {
    ffi.registerCodeCell(code, name);
}

/// Returns current account state for address or none if no information available.
fun net.getAccountState(addr: address): AccountInfo? {
    val cell = ffi.getAccountState(addr.toCell());
    if (cell == null) {
        return null;
    }
    val typed_cell = cell as Cell<AccountInfo>;
    return typed_cell.load()
}

/// Returns storage fee for address for specified amount of seconds.
///
/// Returns `null` if the contract with given address is not deployed.
fun net.getAccountStorageFee(addr: address, seconds: int) {
    val state = net.getAccountState(addr);
    if (state == null) {
        return null;
    }
    val used_storage = state.storage_stat.used;
    return calculateStorageFee(BASECHAIN, seconds, used_storage.bits, used_storage.cells);
}

/// Load library from the real blockchain by hash.
///
/// Returns null if library is not found.
fun net.loadLibrary(hash: slice): cell? {
    return ffi.loadLibrary(hash)
}

/// Load library from the real blockchain by hash and register it in current blockchain.
///
/// Returns false if library is not found.
fun net.fetchAndRegisterLibrary(hash: slice): bool {
    val cell = net.loadLibrary(hash);
    if (cell == null) {
        return false
    }
    vm.registerLibrary(cell);
    return true
}

/// Calculate the CRC16 checksum of a slice.
///
/// Example:
/// ```
/// val crc = crc16("hello");
/// ```
fun crc16(data: slice): int asm "13 EXTCALL"

fun tuple.fromTuple(data: tuple): tuple {
    return data;
}

struct impl

fun impl.runGetMethodById<Ret, Args = tuple>(addr: address, id: int, name: slice, args: Args? = null): Ret {
    if (!net.isDeployed(addr)) {
        throw CANNOT_RUN_GET_METHOD_OD_UNDEPLOYED_CONTRACT;
    }

    val code = net.getDeployedCode(addr);
    if (code == null) {
        throw CANNOT_RUN_GET_METHOD_OF_CONTRACT_WITHOUT_CODE;
    }

    val args_to_send = args == null ? createEmptyTuple() : args.toTuple();
    val res = ffi.runGetMethod(addr.toCell(), code, id, name, Ret.typeName(), args_to_send);
    if (res == null) {
        // actual error will be processed by test framework, just halt execution
        throw 0
    }
    try {
        return Ret.fromTuple(res);
    } catch (e) {
        if (e == 7) {
            Assert.fail(format3("Get method '{}' returned '{}' that cannot be deserialized to '{}'. Make sure you passed all parameters to the get method.", name, res, Ret.typeName()))
        }

        throw e
    }
}

fun __treasuryCode(): cell asm """
    "te6cckEBBAEARQABFP8A9KQT9LzyyAsBAgEgAwIAWvLT/+1E0NP/0RK68qL0BNH4AH+OFiGAEPR4b6UgmALTB9QwAfsAkTLiAbPmWwAE0jD+omUe" base64>B B>boc PUSHREF
"""

fun ffi.sendMessage(msg: cell, mode: int): TransactionList? asm "7 EXTCALL"
fun ffi.sendMessageFrom(msg: cell, from: cell, mode: int): SendResultList? asm "9 EXTCALL"
fun ffi.runGetMethod(addr: cell, code: cell, id: int, name: slice, return_type: slice, args: tuple): tuple? asm "8 EXTCALL"
fun ffi.isDeployed(addr: cell): bool asm "11 EXTCALL"
fun ffi.getDeployedCode(addr: cell): cell? asm "12 EXTCALL"
fun ffi.registerAddress(address: cell, name: slice): void asm "15 EXTCALL"
fun ffi.registerCodeCell(cell: cell, name: slice): void asm "16 EXTCALL"
fun ffi.findTransactionByParams(txs: tuple, params: tuple): cell? asm "10 EXTCALL"
fun ffi.failToFindTransactionByParams(location: slice, message: slice, txs: tuple, params: tuple): tuple? asm "103 EXTCALL"
fun ffi.failToNotFindTransactionByParams(location: slice, message: slice, txs: tuple, params: tuple): tuple? asm "104 EXTCALL"
fun ffi.getAccountState(addr: cell): cell? asm "17 EXTCALL"
fun ffi.loadLibrary(hash: slice): cell? asm "21 EXTCALL"
fun ffi.isBroadcasting(): bool asm "23 EXTCALL"
fun ffi.getWalletByName(name: slice): cell? asm "24 EXTCALL"
fun ffi.waitForTransaction(addr: cell, messageBodyHash: cell, quiet: bool, attempts: int, sleepDuration: int): bool asm "25 EXTCALL"
fun ffi.enableBroadcast(): void asm "26 EXTCALL"
fun ffi.disableBroadcast(): void asm "27 EXTCALL"
