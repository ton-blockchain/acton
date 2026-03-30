/// Module for blockchain emulation.
///
/// This is the central module for interacting with the emulated blockchain environment.
/// It provides capabilities to send messages, manage time, inspect account states, and perform other network-level operations.
///
/// Examples:
/// ```tolk
/// // Setup
/// val deployer = net.wallet("deployer");
/// val contract = build("MyContract");
///
/// // Interact
/// val msg = createMessage({ ... });
/// val results = net.send(deployer.address, msg);
///
/// // Check state
/// val balance = net.balance(contract.address);
/// val seqno = net.runGetMethod<int>(contract.address, "seqno");
/// ```

import "@stdlib/gas-payments"
import "@stdlib/reflection"
import "../fmt"
import "../ffi/ffi"
import "../testing/assert"
import "../tlb/either"
import "../tlb/maybe"
import "../types/big_array"
import "../types/message"
import "../types/out_actions"
import "../types/transaction"
import "../vm/vm"
import "config"

const CANNOT_RUN_GET_METHOD_OD_UNDEPLOYED_CONTRACT = 678
const CANNOT_RUN_GET_METHOD_OF_CONTRACT_WITHOUT_CODE = 679

/// Abstraction representing a blockchain and methods of interacting with it.
struct net

/// Represents a set of parameters for creating a message.
struct SendParams {
    /// Value to send with the message.
    value: coins = ton("0.1")
    /// Whether to bounce the message if the contract does not exist or fails (default: false)
    bounce: bool = false
}

/// Information about external outgoing message.
struct ExtOutMsgInfo {
    /// Internal source address.
    src: address
    /// Optional external destination address.
    dst: address
    /// Logical time when the message was created.
    createdLt: int64
    /// Unix timestamp when the message was created.
    createdAt: uint32
}

/// Represents a list of child transactions IDs (LT) for single send result.
type ChildTxsIds = array<int>

/// Represents a list of out messages for single send result.
type OutMessages = array<cell>

/// Represents a list of external outgoing messages for single send result.
type ExtOutList = array<cell>

/// Returns the external outgoing message at the given index with given type.
///
/// If the message is not of the requested type, loading fails with exit code 63.
/// Use [ExtOutList.atOrNull] to handle this case without throwing.
fun ExtOutList.at<Msg>(self, index: int): ExtMessageRelaxed<Msg> {
    val cell = self.get(index) as cell as Cell<ExtMessageRelaxed<Msg>>;
    return cell.load();
}

/// Returns the external outgoing message at the given index with given type or
/// `null` if message is not of the requested type.
///
/// If message is not of the requested type, `index < 0`, or `index >= self.size()`,
/// this method returns `null` instead of throwing.
/// Use [ExtOutList.at] if you do not want to handle the `null` case.
fun ExtOutList.atOrNull<Msg>(self, index: int): ExtMessageRelaxed<Msg>? {
    if (index < 0 || index >= self.size()) {
        return null;
    }

    val cell = self.get(index) as cell as Cell<ExtMessageRelaxed<Msg>>;
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
    return self.get(index) as cell;
}

/// Returns the out message at the given index as a parsed message of given type.
///
/// To get raw cell, use [OutMessages.atCell] method.
///
/// If actual message body has a different type, loading will fail.
fun OutMessages.at<Msg>(self, index: int): MessageRelaxed<Msg> {
    val cell = self.get(index) as cell as Cell<MessageRelaxed<Msg>>;
    try {
        return cell.load();
    } catch (e) {
        if (e == 63) {
            // prefix mismatch
            val cellGeneric = self.get(index) as cell as Cell<MessageRelaxedGeneric>;
            val msg = cellGeneric.load();

            val opcode = msg.loadOpcode();
            if (opcode == null) {
                throw e;
            }

            val opcodeTypeName = ffi.typeNameByOpcode(opcode);

            Assert.fail(
                format4(
                    "Out message is not a `{}` (opcode: 0x{:x}), but `{}` (opcode: 0x{:x})",
                    reflect.typeNameOf<Msg>(),
                    reflect.serializationPrefixOf<Msg>(),
                    opcodeTypeName == null ? "" : opcodeTypeName,
                    opcode,
                ),
            );
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
    childTxs: ChildTxsIds
    /// Parent transaction LT that produced this transaction or null if this transaction is the root one.
    parentLt: int?
    /// Raw cell with encoded out actions.
    /// Use [SendResult.allOutActions] to load all actions.
    outActions: Cell<OutList>
    /// All out messages produced by this transaction.
    outMessages: OutMessages
    /// Amount of gas used by this transaction.
    gasUsed: int
    /// All external outgoing messages produced by this transaction.
    externals: ExtOutList
}

/// Returns all out actions produced by this transaction as a list.
fun SendResult.allOutActions(self): OutActionList {
    return vm.parseOutActions(self.outActions);
}

/// Represents a list of send results obtained from [net.send] method.
///
/// This list represent all transactions produced by a [net.send] call.
///
/// To get a single send result, use [SendResultList.at] method.
type SendResultList = BigArray<SendResult>

/// Returns the send result at the given index.
fun SendResultList.at(self, index: int): SendResult {
    return self.get(index);
}

/// Opaque cursor for step-by-step transaction execution.
///
/// Use [net.sendIter] to create it, then drain it in parts with [TxCursor.executeN],
/// [TxCursor.executeTill], or [TxCursor.executeFrom].
struct TxCursor {
    id: int
}

/// Returns `true` when the cursor has no more pending transactions to execute.
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
    val declaredPrefix = Msg.getDeclaredPackPrefix2();
    if (declaredPrefix != -1) {
        params.opcode = declaredPrefix;
    }

    val txs = ffi.executeMessageIterTill(self.id, params.toTuple());
    if (txs == null) {
        Assert.fail("executeTill() did not find a matching transaction before queue exhaustion");
        throw 567;
    }

    return txs;
}

/// Executes all remaining pending transactions and returns only this executed segment.
fun TxCursor.executeFrom(self): SendResultList {
    return ffi.executeMessageIterFrom(self.id);
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
/// ```tolk
/// val result = net.send(wallet.address, message);
/// val success = result.wait();
/// if (success) {
///     println("Transaction applied successfully");
/// }
/// ```
///
/// Example with custom parameters:
/// ```tolk
/// val result = net.send(wallet.address, message);
/// val success = result.wait(true, 30, 3000); // quiet mode, 30 attempts, 3 seconds delay
/// ```
fun SendResultList.wait(
    self,
    quiet: bool = false,
    attempts: int = 20,
    sleepDuration: int = 1000,
): bool {
    if (!net.isBroadcasting()) {
        // in emulation mode we don't need to wait for anything
        return true;
    }

    if (self.size() == 0) {
        // unexpected
        return false;
    }

    val res = self.at(0);
    val tx = res.tx.load();
    val inMsgCell = tx.messages.load().inMsg.unwrap();
    val inMsg = inMsgCell.load();
    var body = inMsg.body;
    body.skipBits(1); // skip Either bit
    val bodyHash = body.hash();
    val bodyHashSlice = beginCell().storeUint(bodyHash, 256).toSlice();
    return net.waitForTransaction(inMsg.info.dest, bodyHashSlice, quiet, attempts, sleepDuration);
}

/// Represents a set of parameters to search for a transaction in the transaction list.
struct SearchParams {
    /// Destination address of the transaction.
    to: (address -> bool) | address | null = null
    /// Source address of the transaction.
    from: (address -> bool) | address | null = null
    /// Value of the message in nanotons.
    value: (coins -> bool) | coins | null = null
    /// Exit code of the transaction.
    exitCode: (int32 -> bool) | int32 | null = null
    /// Whether the transaction is successful (both compute and action phase).
    success: (bool -> bool) | bool | null = null
    /// Whether the compute phase of transaction was aborted and transaction was reverted.
    aborted: (bool -> bool) | bool | null = null
    /// Whether the transaction is a deploy transaction.
    ///
    /// Deploy transaction is a transaction that changes the `from` account status from "NonExisting" to "Active".
    deploy: (bool -> bool) | bool | null = null
    /// Whether the transaction message is bounceable.
    bounce: (bool -> bool) | bool | null = null
    /// Whether the transaction is a bounced transaction.
    bounced: (bool -> bool) | bool | null = null
    /// Opcode of the in message.
    opcode: (uint32 -> bool) | uint32 | null = null
    /// Action exit code of the transaction.
    actionExitCode: (int32 -> bool) | int32 | null = null
    /// Whether the compute phase was skipped.
    computePhaseSkipped: (bool -> bool) | bool | null = null
    /// Body of the in message (compared by hash).
    body: (cell -> bool) | cell | null = null
}

/// Internal representation of SearchParams where all fields are predicate functions.
/// Used for FFI communication — plain values are converted to equality predicates.
struct SearchParamsInternal {
    to: (address -> bool)?  = null
    from: (address -> bool)?  = null
    value: (coins -> bool)?  = null
    exitCode: (int32 -> bool)?  = null
    success: (bool -> bool)?  = null
    aborted: (bool -> bool)?  = null
    deploy: (bool -> bool)?  = null
    bounce: (bool -> bool)?  = null
    bounced: (bool -> bool)?  = null
    opcode: (uint32 -> bool)?  = null
    actionExitCode: (int32 -> bool)?  = null
    computePhaseSkipped: (bool -> bool)?  = null
    body: (cell -> bool)?  = null
}

fun cell.__eq(self, other: cell): bool {
    return self.hash() == other.hash();
}

fun T.__eq(self, other: T): bool {
    return self == other;
}

/// Bind first argument to a 2-arg function, producing a 1-arg function.
fun __bind1<T1, T2, R>(f: (T1, T2) -> R, arg: T1): (T2 -> R) asm "SWAP 1 -1 SETCONTARGS";

fun toPredicate<T>(field: (T -> bool) | T | null): (T -> bool)? {
    match (field) {
        (T -> bool) => {
            return field;
        }
        T => {
            return __bind1(field.__eq, field);
        }
        null => {
            return null;
        }
    }
}

fun SearchParams.toInternal(self): SearchParamsInternal {
    return SearchParamsInternal {
        to: toPredicate(self.to),
        from: toPredicate(self.from),
        value: toPredicate(self.value),
        exitCode: toPredicate(self.exitCode),
        success: toPredicate(self.success),
        aborted: toPredicate(self.aborted),
        deploy: toPredicate(self.deploy),
        bounce: toPredicate(self.bounce),
        bounced: toPredicate(self.bounced),
        opcode: toPredicate(self.opcode),
        actionExitCode: toPredicate(self.actionExitCode),
        computePhaseSkipped: toPredicate(self.computePhaseSkipped),
        body: toPredicate(self.body),
    };
}

struct ExternalOutSearchParams {
    /// Source address of the external-out message.
    from: address? = null
    /// Destination address of the external-out message.
    to: any_address? = null
}

fun Message<RemainingBitsAndRefs, ExternalOutMessageInfo>.matches<TBody = unknown>(
    self,
    params: ExternalOutSearchParams = {},
): bool {
    if (params.from != null && self.info.src != params.from) {
        return false;
    }

    if (params.to != null && self.info.dest != params.to) {
        return false;
    }

    val declaredPrefix = TBody.externalOutPrefix();
    if (declaredPrefix != -1) {
        val body = self.loadBody();
        val declaredPrefixLen = TBody.externalOutPrefixLen();
        if (body.remainingBitsCount() < declaredPrefixLen) {
            return false;
        }

        val prefix = body.preloadUint(declaredPrefixLen);
        if (prefix != declaredPrefix) {
            return false;
        }
    }

    return true;
}

/// Find a external-out message in the send result list by specified parameters.
///
/// Example:
/// ```tolk
/// val tx = net.send(deployer.address, message);
/// val result = tx.findExternalOutMessage({ from: counter.address });
/// expect(result).toBeDefined();
/// val tx = result.unwrap();
/// ```
///
/// You can specify an optional type argument, specifying the message type to search for;
/// the function will also check that the opcode of the tx message matches the opcode of the
/// given message.
///
/// Example:
/// ```tolk
/// tx.findExternalOutMessage<IncrementEvent>({ from: counter.address });
/// //                        ^^^^^^^^^^^^^^ search for message with IncrementEvent opcode
/// ```
///
/// If the message is not found, returns None.
fun SendResultList.findExternalOutMessage<TBody = unknown>(
    self,
    params: ExternalOutSearchParams = {},
): Maybe<Message<TBody, ExternalOutMessageInfo>> {
    var transactionIdx = 0;
    while (transactionIdx < self.size()) {
        val externals = self.at(transactionIdx).externals;

        var externalIdx = 0;
        while (externalIdx < externals.size()) {
            val externalOutCell = externals.get(externalIdx) as cell as Cell<
                Message<RemainingBitsAndRefs, ExternalOutMessageInfo>,
            >;
            val externalOut = externalOutCell.load();

            if (externalOut.matches<TBody>(params)) {
                return Maybe<Message<TBody, ExternalOutMessageInfo>>.some(Message {
                    info: externalOut.info,
                    init: externalOut.init,
                    body: EitherLeft { value: TBody.decodeExternalOutBody(externalOut.loadBody()) },
                });
            }

            externalIdx += 1;
        }

        transactionIdx += 1;
    }

    return Maybe<Message<TBody, ExternalOutMessageInfo>>.none();
}

/// Find a transaction in the send result list by specified parameters.
///
/// Example:
/// ```tolk
/// val tx = net.send(deployer.address, message);
/// val result = tx.findTransaction({ to: counter.address });
/// expect(result).toBeDefined();
/// val tx = result.unwrap();
/// ```
fun SendResultList.findTransaction<Msg = unknown>(self, params: SearchParams = {}): Maybe<
    Transaction,
> {
    val declaredPrefix = Msg.getDeclaredPackPrefix2();
    if (declaredPrefix != -1) {
        params.opcode = declaredPrefix;
    }
    val tx = ffi.findTransactionByParams(self, params.toInternal().toTuple());
    if (tx == null) {
        return Maybe<Transaction>.none();
    }
    val txCell = tx as Cell<Transaction>;
    return Maybe<Transaction>.some(txCell.load());
}

/// Send a message from a specified address.
///
/// In broadcast mode, sending to the real network is attempted only when `from`
/// matches one of the wallets opened at script startup.
/// If `from` is not an opened wallet address, this method falls back to normal
/// emulator execution, even when [net.isBroadcasting] is `true`.
///
/// Example:
/// ```tolk
/// get fun `test-send-message`() {
///     // ...
///     val message = createMessage({
///         bounce: false,
///         dest: counter.address,
///         value: ton("1"),
///     });
///     net.send(deployer.address, message);
/// }
/// ```
///
/// Returns a list of send results representing all transactions produced by this send call.
///
/// See [net.sendSingle] if you need to send a single message without out message processing.
fun net.send(from: address, message: OutMessage): SendResultList {
    val rawRes = ffi.sendMessageFrom(message.messageCell, from.toCell());
    if (rawRes == null) {
        // just throw to show original error message from Rust
        throw 100;
    }
    return rawRes;
}

/// Start step-by-step execution for a message sent from [from].
///
/// In contrast to [net.send], this method does not execute the whole transaction chain eagerly.
/// Instead it returns a cursor that can be advanced in batches.
///
/// This API is emulation-only and is intended for programmable partial execution workflows
/// such as large cascades and interleaving multiple message chains.
///
/// Step-by-step execution is not available in broadcast mode or test-runner debug mode.
fun net.sendIter(from: address, message: OutMessage): TxCursor {
    return TxCursor {
        id: ffi.startMessageIter(message.messageCell, from.toCell()),
    };
}

/// Send a single message from a specified address.
///
/// In contrast to [net.send], this method doesn't process out messages, but returns
/// a single send result with this out messages.
///
/// This method is useful for bounce testing where you want to simulate a bounced message.
///
/// Example:
/// ```tolk
/// val bounced_msg = createMessage({
///     bounce: false,
///     value: ton("0.95"),
///     dest: deployer_jetton_wallet.address,
///     body: beginCell().storeUint(0xFFFFFFFF, 32).storeSlice(body.toCell().beginParse()).endCell(),
/// }).bounced();
///
/// net.sendSingle(not_deployer_jetton_wallet.address, bounced_msg);
/// ```
///
/// See [net.send] if you need to send a message and process out messages.
fun net.sendSingle(from: address, message: OutMessage): SendResult {
    val res = ffi.sendSingleMessage(message.messageCell, from.toCell())!;
    return SendResult.fromTuple(res);
}

/// Creates an External-in message.
/// Body is always packed into the Either Right (in the reference).
/// If `StateInit` is present it is also packed in the Either Right (in the reference).
/// Full TL-B scheme of the message is.
/// ```
/// ext_in_msg_info$10 src:MsgAddressExt dest:MsgAddressInt
///   import_fee:Grams = CommonMsgInfo;
/// message$_ {X:Type} info:CommonMsgInfo
///   init:(Maybe (Either StateInit ^StateInit))
///   body:(Either X ^X) = Message X;
///```
fun createExternalMessage<TBody>(
    dest: address,
    body: TBody,
    stateInit: StateInit? = null,
    src: any_address? = null,
): OutMessage {
    val init = stateInit != null
        ? Maybe<Either<StateInit, Cell<StateInit>>>.some(
            Either<StateInit, Cell<StateInit>>.right(stateInit.toCell()),
        )
        : Maybe<Either<StateInit, Cell<StateInit>>>.none();

    // A little hack because Tolk does not support arbitrary function calls as default parameters
    if (src == null) {
        src = createAddressNone();
    }
    // Tolk does not have ext_address type, so ext_address = any_address \ int_address
    if (src.isInternal()) {
        Assert.fail("Internal source address is not supported for External-in messages");
    }

    return OutMessage {
        messageCell: Message<TBody, ExternalInMessageInfo> {
            info: ExternalInMessageInfo {
                src,
                dest,
                // It is better to fairly calculate the import fee right here or in acton
                importFee: ton("0.1"),
            },
            init,
            body: Either<TBody, Cell<TBody>>.right(body.__asPayload()),
        }.toCell(),
    };
}

/// Sends an external incoming message to the specified destination address.
/// Read more about external incoming messages: https://docs.ton.org/foundations/messages/external-in#incoming-external-messages
/// Returns `null` if external message was not accepted or destination account does not exist.
fun net.sendExternal(msg: OutMessage): SendResultList? {
    // from and mode fields do not affect anything in case of an ExtIn message
    return ffi.sendMessageFrom(
        msg.messageCell,
        address("-1:0000000000000000000000000000000000000000000000000000000000000000").toCell(),
    );
}

/// Runs a tick or tock transaction on the specified account.
///
/// Tick-tock transactions have no incoming message and are triggered by the blockchain
/// on special accounts (e.g., elector, config).
///
/// Example:
/// ```tolk
/// val results = net.runTickTock(electorAddress, false); // run tick
/// val results = net.runTickTock(electorAddress, true);  // run tock
/// ```
fun net.runTickTock(onAccount: address, isTock: bool): SendResultList {
    return ffi.runTickTock(onAccount, isTock);
}

/// Transform the out message to a bounced message. Essentially, it sets the `bounced` flag to true.
///
/// Example:
/// ```tolk
/// val msg = createMessage({
///     bounce: false,
///     value: ton("1"),
///     dest: counter.address,
/// });
/// val bouncedMsg = msg.bounced();
/// ```
///
/// This is useful for testing single messages with [net.sendSingle] method.
///
/// Example:
/// ```tolk
/// val bounced_msg = createMessage({
///     bounce: false,
///     value: ton("0.95"),
///     dest: deployer_jetton_wallet.address,
///     body: beginCell().storeUint(0xFFFFFFFF, 32).storeSlice(body.toCell().beginParse()).endCell(),
/// }).bounced();
///
/// net.sendSingle(not_deployer_jetton_wallet.address, bounced_msg);
/// ```
fun OutMessage.bounced(self): OutMessage {
    var msg = (self.messageCell as Cell<MessageRelaxedGeneric>).load();
    msg.info.bounced = true;
    return { messageCell: msg.toCell() };
}

/// Treasury is a special contract that is used to deploy other contracts.
///
/// To create a treasury, use the [net.treasury] method.
struct Treasury {
    address: address
}

/// Create a treasury with a specified name.
///
/// Calling this method with same name will return the same treasury.
///
/// Example:
/// ```tolk
/// get fun `test-mint`() {
///     val treasury = net.treasury("treasury");
///     // ...
/// }
/// ```
///
/// Note: in scripts always use [net.wallet] instead.
fun net.treasury(name: string): Treasury {
    if (net.isBroadcasting()) {
        Assert.fail("Cannot use treasury in broadcast mode, don't use `net.treasury()` in scripts, use `net.wallet()` instead")
    }

    val stateInit: ContractState = {
        code: __treasuryCode(),
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
/// Note: in tests, use [net.treasury].
///
/// Example:
/// ```tolk
/// /// Script that deploys a counter contract using a wallet to real blockchain.
/// fun main() {
///    val deployer = net.wallet("deployer");
///    val counter = Counter.fromStorage(Storage {
///        id: 8,
///        counter: 0,
///    });
///
///    val result = counter.deploy(deployer.address, ton("0.05"));
///    result.wait();
///
///    println1("Deployed counter to {}", counter.address);
/// }
/// ```
fun net.wallet(name: string): Wallet {
    if (ffi.isBroadcasting()) {
        val addressCell = ffi.getWalletByName(name);
        if (addressCell == null) {
            Assert.failWalletNotFound(name);
            throw 1;
        }
        val address = addressCell.beginParse().loadAddress();
        return Wallet { address };
    }

    return Wallet { address: net.treasury(name).address };
}

/// Check if the script is running in broadcasting mode.
///
/// Returns `true` if the script was run with `--broadcast` flag, `false` otherwise.
///
/// This is useful for writing scripts that behave differently in emulation and broadcasting modes.
///
/// Example:
/// ```tolk
/// if (net.isBroadcasting()) {
///     println("Running on real network");
/// } else {
///     println("Running in emulation mode");
/// }
/// ```
fun net.isBroadcasting(): bool {
    return ffi.isBroadcasting();
}

/// Enable broadcasting mode.
///
/// This function allows scripts to manually enable broadcasting behavior.
/// Useful for testing broadcasting logic in emulation mode.
/// It toggles only the runtime broadcast flag.
///
/// Wallets are loaded once at startup (when the script is launched with CLI `--broadcast`).
/// Calling [net.enableBroadcast] later does not load wallets, so [net.wallet] can still fail
/// and wallet-backed real sends can remain unavailable.
///
/// Example:
/// ```tolk
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
/// It does not reload or rebuild the startup wallet map.
///
/// Example:
/// ```tolk
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
/// ```tolk
/// get fun get_balance(): coins { ... }
/// ```
///
/// To call it, you can use the following code:
/// ```tolk
/// val balance = net.runGetMethod<coins>(wallet.address, "get_balance");
/// ```
/// Since this get method doesn't accept any arguments, third argument can be omitted.
///
/// ## With arguments
///
/// If the get method accepts single argument, you can pass it as a third argument.
/// ```tolk
/// get fun get_wallet_address(ownerAddress: address): address { ... }
/// ```
/// ```tolk
/// val address = net.runGetMethod<address>(wallet.address, "get_wallet_address", deployer.address);
/// ```
///
/// If the get method accepts several arguments, you can pass them as a third argument in tuple.
/// ```tolk
/// get fun do_smth(owner: address, id: int): address { ... }
/// ```
/// ```tolk
/// val address = net.runGetMethod<address>(wallet.address, "do_smth", [deployer.address, 10] as tuple);
/// ```
///
/// ## With struct return type
///
/// If get method returns a struct, specify the type of the struct as a first type argument:
/// ```tolk
/// get fun get_jetton_data(): JettonDataReply { ... }
/// ```
/// ```tolk
/// val data = net.runGetMethod<JettonDataReply>(wallet.address, "get_jetton_data");
/// ```
///
/// ## With tensor return type
///
/// If get method returns a tensor, specify tuple as a first type argument:
/// ```tolk
/// get fun get_all_data(): (int, slice) { ... }
/// ```
/// ```tolk
/// val data: tuple = net.runGetMethod(wallet.address, "get_all_data");
/// val first = data.get(0) as int;
/// val second = data.get(1) as int;
/// ```
///
/// If you need to run a get method by its ID, use [net.runGetMethodById] instead.
fun net.runGetMethod<Ret, Args = tuple>(addr: address, name: string, args: Args? = null): Ret {
    val methodId = (crc16(name) & 0xffff) | 0x10000;
    return impl.runGetMethodById(addr, methodId, name, args);
}

/// Run a get method of a contract by its ID.
///
/// If you need to run a get method by its name, use [net.runGetMethod] instead.
///
/// Example:
/// ```tolk
/// val data = net.runGetMethodById<JettonDataReply>(wallet.address, 0x17b02);
/// ```
///
/// See [net.runGetMethod] for more information.
fun net.runGetMethodById<Ret, Args = tuple>(addr: address, id: int, args: Args? = null): Ret {
    return impl.runGetMethodById(addr, id, "", args);
}

/// Check if a contract with a specified address is deployed.
///
/// Returns `true` if the contract is deployed, `false` otherwise.
///
/// Example:
/// ```tolk
/// fun JettonWallet.getJettonBalance(self): coins {
///     if (!net.isDeployed(self.address)) {
///         return 0 // return 0 if contract is not deployed
///     }
///
///     // and if deployed, actually run get method
///     val result: JettonWalletDataReply = net.runGetMethod(self.address, "get_wallet_data");
///     return result.jettonBalance
/// }
/// ```
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
/// For `custom:<name>` and `localnet` networks, transaction links in output
/// use `[networks.<name>].explorer` from `Acton.toml` as base URL.
/// Acton appends `/tx/<hash>` automatically (adds `/tx/` if missing).
/// If `explorer` is not set, Acton derives link from `api.v2` as
/// `<scheme>://<host>/explorer/tx/<hash>`.
/// For default `localnet`, this becomes
/// `http://localhost:<litenode.port-or-5411>/explorer/tx/<hash>`.
///
/// Example:
/// ```tolk
/// val txs = net.send(wallet.address, message);
/// val tx = txs.at(0).tx.load();
/// val inMsg = tx.messages.load().inMsg.unwrap().load();
///
/// var body = inMsg.body;
/// body.skipBits(1); // skip Either tag bit
/// val bodyHashSlice = beginCell().storeUint(body.hash(), 256).toSlice();
///
/// val ok = net.waitForTransaction(inMsg.info.dest, bodyHashSlice, true, 20, 1000);
/// ```
///
/// See [SendResultList.wait] for high-level API.
fun net.waitForTransaction(
    addr: address,
    messageBodyHash: slice,
    quiet: bool = false,
    attempts: int = 20,
    sleepDuration: int = 1000,
): bool {
    return ffi.waitForTransaction(
        addr.toCell(),
        messageBodyHash.toCell(),
        quiet,
        attempts,
        sleepDuration,
    );
}

/// Generate a random address.
///
/// If a name is provided, the address will be registered with the given name.
/// This allows test runner to show the name instead of the address in the test output.
///
/// Example:
/// ```tolk
/// val address = net.randomAddress("random_user");
/// val masterchain_address = net.randomAddress("random_user", -1);
/// ```
fun net.randomAddress(name: string? = null, wc: int = 0): address {
    var builder = beginCell();
    builder.storeUint(0b10, 2);
    builder.storeUint(0b0, 1);
    builder.storeInt(wc, 8);

    if (name != null) {
        val deterministicHash = beginCell()
            .storeInt(wc, 8)
            .storeSlice(name.beginParse())
            .endCell()
            .hash();
        builder.storeUint(deterministicHash, 256);
    } else {
        var i = 0;
        while (i < 32) {
            val num = random.range(256);
            builder.storeUint(num, 8);
            i += 1;
        }
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
/// ```tolk
/// net.registerAddress(address, "my_address");
/// ```
fun net.registerAddress(addr: address, name: string): void {
    ffi.registerAddress(addr.toCell(), name);
}

/// Register a code cell with a name.
///
/// This allows test runner to show the name instead of the address in the test output
/// if the actual code of the contract is match to previously registered code.
///
/// Example:
/// ```tolk
/// net.registerCodeCell(code, "my_code");
/// ```
fun net.registerCodeCell(code: cell, name: string): void {
    ffi.registerCodeCell(code, name);
}

/// Returns current account state for address or null if no information available or
/// contract is not deployed.
///
/// The account state includes balance, storage information, and other account data,
/// see [AccountInfo] type for more information.
///
/// Example:
/// ```tolk
/// val state = net.getAccountState(contract.address);
/// if (state != null) {
///     println1("Balance: {:ton}", state.storage.balance.grams);
/// } else {
///     println("Account not found");
/// }
/// ```
fun net.getAccountState(addr: address): AccountInfo? {
    val cell = ffi.getAccountState(addr.toCell());
    if (cell == null) {
        return null;
    }
    val typedCell = cell as Cell<AccountInfo>;
    return typedCell.load();
}

/// Returns storage fee for address for specified amount of seconds.
///
/// Returns `null` if the contract with given address is not deployed.
///
/// Example:
/// ```tolk
/// val storage_fee = net.getAccountStorageFee(contract.address, 86400); // 1 day
/// if (storage_fee != null) {
///     println1("Daily storage fee: {:ton}", storage_fee);
/// }
/// ```
fun net.getAccountStorageFee(addr: address, seconds: int): coins? {
    val state = net.getAccountState(addr);
    if (state == null) {
        return null;
    }
    val usedStorage = state.storageStat.used;
    return calculateStorageFee(BASECHAIN, seconds, usedStorage.bits, usedStorage.cells);
}

/// Load library from the real blockchain by hash.
///
/// Returns null if library is not found.
fun net.loadLibrary(hash: string): cell? {
    return ffi.loadLibrary(hash);
}

/// Load library from the real blockchain by hash and register it in current blockchain.
///
/// Returns false if library is not found.
fun net.fetchAndRegisterLibrary(hash: string): bool {
    val cell = net.loadLibrary(hash);
    if (cell == null) {
        return false;
    }
    vm.registerLibrary(cell);
    return true;
}

/// Set the current Unix timestamp for time forwarding emulation.
///
/// This is useful for testing time-dependent contracts.
///
/// Example:
/// ```tolk
/// val future_time = net.now() + 1000; // 1000 seconds from now
/// net.setNow(future_time);
/// ```
fun net.setNow(now: uint32): void {
    ffi.setNow(now);
}

/// Get the current Unix timestamp used in emulation.
///
/// Returns the current time in the emulated blockchain.
///
/// Example:
/// ```tolk
/// val current_time = net.now();
/// println1("Current emulation time: {}", current_time);
/// ```
fun net.now(): uint32 {
    return ffi.getNow();
}

/// Topup an address with a specified amount of TON.
///
/// Example:
/// ```tolk
/// net.topUp(address, ton("10"));
/// ```
///
/// Note: This function for tests only!
/// For testnet TON airdrop use `acton wallet airdrop command.
fun net.topUp(to: address, amount: coins): void {
    val treasury = net.treasury("treasury");
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
/// val balance = net.balance(address);
/// println1("{:ton}", balance);
/// ```
fun net.balance(of: address): coins {
    val state = net.getAccountState(of);
    if (state == null) {
        return 0;
    }
    return state.storage.balance.grams;
}

/// Returns the account state for an address in emulation.
///
/// The returned value is either `AccountInfo` for an existing account
/// or `AccountNone` if the account does not exist.
///
/// Example:
/// ```tolk
/// val acc = net.getAccount(addr);
/// if (acc is AccountInfo) {
///     println("Account exists");
/// }
/// ```
fun net.getAccount(addr: address): Account {
    val shard = net.getShardAccount(addr);
    return shard!.account.load();
}

/// Sets the account state for an address in emulation.
///
/// If the account already exists, preserves `lastTransHash` and `lastTransLt`
/// from the existing shard account. Otherwise, initializes them to zero.
///
/// Example:
/// ```tolk
/// val acc = net.getAccount(addr);
/// net.setAccount(addr, acc);
/// ```
fun net.setAccount(addr: address, acc: Account): void {
    val shardAcc = net.getShardAccount(addr);
    var lastTransHash = 0;
    var lastTransLt = 0;
    if (shardAcc != null) {
        lastTransHash = shardAcc.lastTransHash;
        lastTransLt = shardAcc.lastTransLt;
    }
    val newShardAcc = ShardAccount {
        account: acc.toCell(),
        lastTransHash,
        lastTransLt,
    };
    net.setShardAccount(addr, newShardAcc);
}

/// Returns the shard account state for an address in emulation.
///
/// The returned shard account can represent an uninitialized account.
///
/// Example:
/// ```tolk
/// val shard = net.getShardAccount(addr);
/// if (shard != null) {
///     println1("Last LT: {}", shard.lastTransLt);
/// }
/// ```
fun net.getShardAccount(addr: address): ShardAccount? {
    val rawRes = ffi.getShardAccount(addr);
    if (rawRes == null) {
        return null;
    }
    return ShardAccount.fromCell(rawRes);
}

/// Sets the shard account state for an address in emulation.
///
/// Pass `null` to reset the account to an empty state.
///
/// Example:
/// ```tolk
/// val shard = net.getShardAccount(addr);
/// if (shard != null) {
///     net.setShardAccount(addr, shard);
/// }
/// ```
fun net.setShardAccount(addr: address, account: ShardAccount?): void {
    if (account == null) {
        ffi.setShardAccount(addr, null);
        return;
    }
    ffi.setShardAccount(addr, account.toCell());
}

/// Save the current emulation world state to a JSON snapshot file.
///
/// Returns `true` on successful write and `false` on serialization or host I/O failure.
///
/// Example:
/// ```tolk
/// net.topUp(addr, ton("3"));
/// val saved = net.saveSnapshot("world-state.json");
/// expect(saved).toBeTrue();
/// ```
fun net.saveSnapshot(path: string): bool {
    return ffi.saveWorldStateSnapshot(path);
}

/// Load an emulation world state from a JSON snapshot file.
///
/// On success, replaces the current local emulation state with the snapshot contents.
/// Returns `false` when the file is missing, malformed, or cannot be applied.
///
/// Example:
/// ```tolk
/// val loaded = net.loadSnapshot("world-state.json");
/// expect(loaded).toBeTrue();
/// ```
fun net.loadSnapshot(path: string): bool {
    return ffi.loadWorldStateSnapshot(path);
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
/// - `config`: The new [Config] map containing configuration parameters (indices 0-90)
///
/// Returns: `true` if the configuration was successfully updated, `false` if validation failed.
///
/// Example:
/// ```tolk
/// var config = net.getConfig();
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
/// val success = net.setConfig(config);
/// if (success) {
///     println("Configuration updated successfully");
/// }
/// ```
fun net.setConfig(config: Config): bool {
    val root = config.toLowLevelDict();
    val result = ffi.setConfig(root);

    if (result) {
        vm.setConfigRootDict(root);

        var unpacked = vm.getConfigUnpacked();

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

        vm.setConfigUnpacked(unpacked);
    }

    return result;
}

/// Retrieves the current blockchain configuration.
///
/// Returns a map of all blockchain configuration parameters indexed from 0-90.
/// The configuration includes gas prices, storage costs, validator settings, and other
/// network parameters. Use specialized getter functions like [Config.getGasPrices],
/// [Config.getStoragePrices], or [Config.getGlobalVersion] to access specific parameters.
///
/// For detailed information about blockchain configuration parameters, see the
/// [TON Blockchain Configuration Reference](https://docs.ton.org/foundations/config).
///
/// Returns: A [Config] map containing all current blockchain configuration parameters.
///
/// Example:
/// ```tolk
/// val config = net.getConfig();
///
/// // Access specific configuration
/// val gasPrice = config.getGasPrices(BASECHAIN);
/// val storagePrices = config.getStoragePrices();
/// val globalVersion = config.getGlobalVersion();
///
/// println(format1("Network version: {}", globalVersion.version));
/// ```
fun net.getConfig(): Config {
    return createMapFromLowLevelDict(ffi.getConfig());
}

/// Calculate the CRC16 checksum of a slice.
///
/// Example:
/// ```tolk
/// val crc = crc16("hello");
/// ```
fun crc16(data: string): int asm "13 EXTCALL"

fun tuple.fromTuple(data: tuple): tuple {
    return data;
}

fun T.externalOutPrefix(): int {
    return reflect.serializationPrefixOf<T>();
}

fun unknown.externalOutPrefix(): int {
    return -1;
}

fun T.externalOutPrefixLen(): int {
    return reflect.serializationPrefixLenOf<T>();
}

fun unknown.externalOutPrefixLen(): int {
    return 0;
}

fun T.decodeExternalOutBody(body: RemainingBitsAndRefs): T {
    return T.fromSlice(body);
}

fun unknown.decodeExternalOutBody(_: RemainingBitsAndRefs): never {
    throw 0;
}

struct impl

fun impl.runGetMethodById<Ret, Args = tuple>(
    addr: address,
    id: int,
    name: string,
    args: Args? = null,
): Ret {
    if (!net.isDeployed(addr)) {
        throw CANNOT_RUN_GET_METHOD_OD_UNDEPLOYED_CONTRACT;
    }

    val code = net.getDeployedCode(addr);
    if (code == null) {
        throw CANNOT_RUN_GET_METHOD_OF_CONTRACT_WITHOUT_CODE;
    }

    val argsToSend = args == null ? [] : args.toTuple();
    val res = ffi.runGetMethod(
        addr.toCell(),
        code,
        id,
        name,
        reflect.typeNameOf<Ret>(),
        argsToSend,
    );
    if (res == null) {
        // actual error will be processed by test framework, just halt execution
        throw 0;
    }
    try {
        return Ret.fromTuple(res);
    } catch (e) {
        if (e == 7) {
            Assert.fail(
                format3(
                    "Get method '{}' returned '{}' that cannot be deserialized to '{}'. Make sure you passed all parameters to the get method.",
                    name,
                    res,
                    reflect.typeNameOf<Ret>(),
                ),
            );
        }

        throw e;
    }
}

fun T.getDeclaredPackPrefix2(): int {
    return reflect.serializationPrefixOf<T>();
}

fun unknown.getDeclaredPackPrefix2(): int {
    return -1;
}

// This function is used in createExternalMessage to not wrap cells twice
fun T.__asPayload(self): Cell<T> {
    return self.toCell();
}

fun Cell<T>.__asPayload(self): never {
    Assert.fail("Can't pass Cell<T> to .createExternalMessage(). Pass untyped cell or T");
}

fun cell.__asPayload(self): Cell<cell> {
    return self as Cell<cell>;
}

fun __treasuryCode(): cell asm """
    "te6cckEBBAEARQABFP8A9KQT9LzyyAsBAgEgAwIAWvLT/+1E0NP/0RK68qL0BNH4AH+OFiGAEPR4b6UgmALTB9QwAfsAkTLiAbPmWwAE0jD+omUe" base64>B B>boc PUSHREF
"""

fun ffi.sendMessageFrom(msg: cell, from: cell): SendResultList? asm "9 EXTCALL"
fun ffi.sendSingleMessage(msg: cell, from: cell): tuple? asm "30 EXTCALL"
fun ffi.runGetMethod(addr: cell, code: cell, id: int, name: string, returnType: string, args: tuple): tuple? asm "8 EXTCALL"
fun ffi.isDeployed(addr: cell): bool asm "11 EXTCALL"
fun ffi.getDeployedCode(addr: cell): cell? asm "12 EXTCALL"
fun ffi.registerAddress(address: cell, name: string): void asm "15 EXTCALL"
fun ffi.registerCodeCell(cell: cell, name: string): void asm "16 EXTCALL"
fun ffi.findTransactionByParams(txs: SendResultList, params: tuple): cell? asm "10 EXTCALL"
fun ffi.failToFindTransactionByParams(location: string, message: string, txs: SendResultList, params: tuple): tuple? asm "103 EXTCALL"
fun ffi.failToNotFindTransactionByParams(location: string, message: string, txs: SendResultList, params: tuple): tuple? asm "104 EXTCALL"
fun ffi.getAccountState(addr: cell): cell? asm "17 EXTCALL"
fun ffi.loadLibrary(hash: string): cell? asm "21 EXTCALL"
fun ffi.isBroadcasting(): bool asm "23 EXTCALL"
fun ffi.getWalletByName(name: string): cell? asm "24 EXTCALL"
fun ffi.waitForTransaction(addr: cell, messageBodyHash: cell, quiet: bool, attempts: int, sleepDuration: int): bool asm "25 EXTCALL"
fun ffi.enableBroadcast(): void asm "26 EXTCALL"
fun ffi.disableBroadcast(): void asm "27 EXTCALL"
fun ffi.setNow(now: int): void asm "28 EXTCALL"
fun ffi.getNow(): int asm "29 EXTCALL"
fun ffi.getConfig(): dict asm "31 EXTCALL"
fun ffi.setConfig(config: dict): bool asm "32 EXTCALL"
fun ffi.getShardAccount(addr: address): cell? asm "33 EXTCALL";
fun ffi.setShardAccount(addr: address, account: cell?): void asm "34 EXTCALL";
fun ffi.saveTraceName(txs: SendResultList, name: string): void asm "35 EXTCALL";
fun ffi.runTickTock(onAccount: address, isTock: bool): SendResultList asm "36 EXTCALL";
fun ffi.saveWorldStateSnapshot(path: string): bool asm "37 EXTCALL";
fun ffi.loadWorldStateSnapshot(path: string): bool asm "38 EXTCALL";
fun ffi.startMessageIter(msg: cell, from: cell): int asm "39 EXTCALL";
fun ffi.executeMessageIterN(cursorId: int, n: int): SendResultList asm "40 EXTCALL";
fun ffi.executeMessageIterTill(cursorId: int, params: tuple): SendResultList? asm "41 EXTCALL";
fun ffi.executeMessageIterFrom(cursorId: int): SendResultList asm "42 EXTCALL";
fun ffi.isMessageIterDone(cursorId: int): bool asm "43 EXTCALL";
fun ffi.closeMessageIter(cursorId: int): void asm "44 EXTCALL";
