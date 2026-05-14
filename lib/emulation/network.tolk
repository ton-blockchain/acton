/// Module for sending messages to a real or emulated blockchain.
///
/// It provides capabilities to send messages to contracts, calling their getters,
/// and performing other network-level operations.
///
/// Note: while writing tests, also use `testing.xxx` methods.
/// While writing scripts, also use `scripts.xxx` methods.
///
/// Examples:
/// ```tolk
/// // Setup
/// val deployer = scripts.wallet("deployer");
/// val contract = build("MyContract");
///
/// // Interact
/// val msg = createMessage({ ... });
/// val txs = net.send(deployer.address, msg);
///
/// // Check state
/// val balance = scripts.fetchAccountBalance(contract.address);
/// val seqno = net.runGetMethod<int>(contract.address, "seqno");
/// ```

import "@stdlib/reflection"
import "../ffi"
import "../fmt"
import "../impl"
import "../testing/assert"
import "../testing/expect"
import "../tlb/either"
import "../tlb/maybe"
import "../types/big_array"
import "../types/message"
import "../types/out_actions"
import "../types/transaction"

/// Root module for sending messages to a real or emulated blockchain.
struct net

const CANNOT_RUN_GET_METHOD_OF_UNDEPLOYED_CONTRACT = 678
const CANNOT_RUN_GET_METHOD_OF_CONTRACT_WITHOUT_CODE = 679

/// Represents a set of parameters for creating a message.
struct SendParams {
    /// Value to send with the message.
    value: coins = ton("0.1")
    /// Whether to bounce the message if the contract does not exist or fails (default: false)
    bounce: bool = false
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
fun ExtOutList.at<Msg>(self, index: int): TlbExtMessageRelaxed<Msg> {
    val cell = self.get(index) as cell as Cell<TlbExtMessageRelaxed<Msg>>;
    return cell.load();
}

/// Returns the external outgoing message at the given index with given type or
/// `null` if message is not of the requested type.
///
/// If message is not of the requested type, `index < 0`, or `index >= self.size()`,
/// this method returns `null` instead of throwing.
/// Use [ExtOutList.at] if you do not want to handle the `null` case.
fun ExtOutList.atOrNull<Msg>(self, index: int): TlbExtMessageRelaxed<Msg>? {
    if (index < 0 || index >= self.size()) {
        return null;
    }

    val cell = self.get(index) as cell as Cell<TlbExtMessageRelaxed<Msg>>;
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
fun OutMessages.at<Msg>(self, index: int): TlbMessageRelaxed<Msg> {
    val cell = self.get(index) as cell as Cell<TlbMessageRelaxed<Msg>>;
    try {
        return cell.load();
    } catch (e) {
        if (e == 63) {
            // prefix mismatch
            val cellGeneric = self.get(index) as cell as Cell<TlbMessageRelaxedGeneric>;
            val msg = cellGeneric.load();

            val opcode = msg.loadOpcode();
            if (opcode == null) {
                throw e;
            }

            val opcodeTypeName = ffi.typeNameByOpcode(opcode);

            Assert.fail(
                format(
                    "Out message is not a `{}` (opcode: 0x{:x}), but `{}` (opcode: 0x{:x})",
                    reflect.typeNameOf<Msg>(),
                    reflect.serializationPrefixOf<Msg>().0,
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
    tx: Cell<TlbTransaction>
    /// Logical time of the transaction.
    lt: int
    /// List of child transactions IDs.
    /// This transactions are directly produced by this transaction.
    childTxs: ChildTxsIds
    /// Parent transaction LT that produced this transaction or null if this transaction is the root one.
    parentLt: int?
    /// Raw cell with encoded out actions.
    /// Use [SendResult.allOutActions] to load all actions.
    outActions: Cell<TlbOutList>
    /// All out messages produced by this transaction.
    outMessages: OutMessages
    /// Amount of gas used by this transaction.
    gasUsed: int
    /// All external outgoing messages produced by this transaction.
    externals: ExtOutList
}

/// Returns all out actions produced by this transaction as a list.
fun SendResult.allOutActions(self): OutActionList {
    return impl.parseOutActions(self.outActions);
}

/// Represents a list of send results obtained from [net.send] method.
///
/// This list represents all transactions produced by a [net.send] call.
///
/// You can print the whole list with [println] to inspect the transaction tree produced by
/// the send call.
///
/// Example output:
/// ```acton-trace
/// N/A -> sender A
/// └── AskToTransfer 0.3 TON -> JettonWallet B                                  gas=5158
///     └── InternalTransferStep 0.19 TON -> JettonWallet C                      gas=6974
///         ├── TransferNotificationForRecipient 0.05 TON -> receiver D          gas=1234
///         └── ReturnExcessesBack 0.13 TON -> sender A                          gas=4321
/// ```
///
/// To get a single send result, use [SendResultList.at] method.
type SendResultList = BigArray<SendResult>

/// Metadata returned when an external-in message fails before producing an
/// accepted transaction trace.
struct ExternalSendError {
    /// Error message returned by the emulator.
    message: string
    /// Whether the external-in message was not accepted by the smart contract.
    externalNotAccepted: bool
    /// VM exit code reported by the emulator, if available.
    vmExitCode: int?
    /// Emulator elapsed time in nanoseconds, if available.
    elapsedTimeNs: int?
    /// Hashes of libraries that were requested but not found during emulation.
    missingLibraries: array<string>
    /// Opaque diagnostic id.
    diagnosticId: int?
}

/// Result returned by [net.sendExternal].
///
/// If the external-in message is accepted, `transactions` contains the produced
/// trace. If it is rejected before acceptance or cannot be emulated,
/// `transactions` is `null` and `error` contains emulator metadata such as VM
/// exit code, elapsed time, missing libraries, and an opaque diagnostic id.
///
/// In tests, prefer `expect(result).toBeAccepted()`,
/// `expect(result).toBeNotAccepted()`, and
/// `expect(result).toEndWithExitCode(...)` for diagnostics.
/// Use [ExternalSendResult.unwrap] when a trace is required, or
/// [ExternalSendResult.isAccepted] when the test needs to branch on the result.
struct ExternalSendResult {
    /// Produced transaction trace, or `null` if the external-in message was not accepted.
    transactions: SendResultList?
    /// Failure metadata, or `null` when the external-in message was accepted.
    error: ExternalSendError?
    /// Destination account extracted from the original external-in message.
    destination: address
}

/// Returns the send result at the given index.
fun SendResultList.at(self, index: int): SendResult {
    return self.get(index);
}

/// Returns `true` when the external-in message produced a transaction trace.
fun ExternalSendResult.isAccepted(self): bool {
    return self.transactions != null;
}

/// Returns produced transactions, or fails with external-error metadata if the
/// message was not accepted.
///
/// Failed unwraps are reported like `expect(result).toBeAccepted()`:
/// diagnostics include the external status, emulator reason, VM exit code when
/// available, known exit-code description or ABI error name, source location,
/// and the full backtrace when the test is run with `--backtrace full`.
///
/// The `location` parameter is filled automatically and is only used for
/// diagnostics; user code should normally omit it.
fun ExternalSendResult.unwrap(
    self,
    location: string = reflect.sourceLocationAsString(),
): SendResultList {
    if (self.transactions != null) {
        return self.transactions!;
    }
    self.__failExternalSend("ExternalSendResult.unwrap()", location);
}

fun ExternalSendError.__toFFITuple(self, destination: address?): tuple {
    return [
        self.message,
        self.externalNotAccepted,
        self.vmExitCode,
        self.elapsedTimeNs,
        self.missingLibraries,
        self.diagnosticId,
        destination,
    ];
}

fun ExternalSendResult.__failExternalSend(self, context: string, location: string = ""): never {
    if (self.error != null) {
        ffi.failExternalSendNotAccepted(
            location,
            context,
            self.error!.__toFFITuple(self.destination),
        );
        throw ASSERTION_FAILED;
    }

    Assert.fail(format("{}: external message was not accepted", context), location);
}

/// Returns the send result at the given index, or fails with external-error
/// metadata if the message was not accepted.
///
/// The `location` parameter is filled automatically and is only used for
/// diagnostics; user code should normally omit it.
fun ExternalSendResult.at(
    self,
    index: int,
    location: string = reflect.sourceLocationAsString(),
): SendResult {
    return self.unwrap(location).at(index);
}

/// Give a custom name for a trace chain produced by [net.sendExternal].
///
/// Fails with external-error metadata if the message was not accepted.
///
/// The `location` parameter is filled automatically and is only used for
/// diagnostics; user code should normally omit it.
fun ExternalSendResult.giveName(
    self,
    nameForUI: string,
    location: string = reflect.sourceLocationAsString(),
): void {
    self.unwrap(location).giveName(nameForUI);
}

/// Give a custom name for a trace chain produced by [net.send].
///
/// The custom name is used in the Test UI and gas/fee profiling reports instead of "trace N".
///
/// Example:
/// ```tolk
/// val txs = net.send(deployer.address, msg);
/// txs.giveName("deploy-counter");
/// expect(txs).toHaveAllSuccessfulTxs();
/// ```
fun SendResultList.giveName(self, nameForUI: string): void {
    ffi.saveTraceName(self, nameForUI);
}

/// Wait for the first (root) message of the send result to be applied on the blockchain.
///
/// This method is useful when broadcasting transactions to the real network.
/// It polls the network until the transaction is applied or the maximum number of attempts is reached.
///
/// The lookup follows the TON docs message-lookup reference
/// (https://docs.ton.org/ecosystem/ton-connect/message-lookup): the external-in message
/// actually delivered to the network is identified by its TEP-467 normalized hash
/// (returned by `sendBocReturnHash` at send time), and the destination account's recent
/// transactions are scanned until one with a matching `inMessage` shows up.
///
/// Returns the matched [SendResult] rebuilt from the on-chain transaction if found, or `null`
/// otherwise. Fields that cannot be reconstructed from a single on-chain transaction
/// (`childTxs`, `parentLt`, `outActions`) are left empty/null; `outMessages`, `externals`,
/// and `gasUsed` are derived from the transaction itself.
///
/// In emulation mode (without `--net`), this method returns the first [SendResult] immediately
/// (or `null` if the list is empty), because the trace is already fully processed locally.
///
/// Example:
/// ```tolk
/// val result = net.send(wallet.address, message);
/// val applied = result.waitForFirstTransaction();
/// if (applied != null) {
///     println("Transaction applied successfully at lt {}", applied.lt);
/// }
/// ```
///
/// Example with custom parameters:
/// ```tolk
/// val result = net.send(wallet.address, message);
/// result.waitForFirstTransaction(true, 30, 3000); // quiet mode, 30 attempts, 3 seconds delay
/// ```
fun SendResultList.waitForFirstTransaction(
    self,
    quiet: bool = false,
    attempts: int = 20,
    sleepDuration: int = 1000,
): SendResult? {
    if (self.size() == 0) {
        return null;
    }

    val res = self.at(0);
    if (!ffi.isBroadcasting()) {
        // in emulation mode the trace is already applied locally
        return res;
    }

    // FFI returns a single-element list (or null) rather than `SendResult?` directly
    // because Tolk packs a struct across multiple stack slots when returned from an asm
    // function, whereas BigArray items are kept on a single slot.
    val list = ffi.waitForTransaction(
        res.tx as cell,
        quiet,
        attempts,
        sleepDuration,
    );
    if (list == null || list.size() == 0) {
        return null;
    }
    return list.at(0);
}

/// Wait for the full message chain originating from the first message to settle on-chain.
///
/// Unlike [SendResultList.waitForFirstTransaction], this method follows all descendant
/// transactions (children, grandchildren, ...) using toncenter's traces API and returns
/// them as a fresh [SendResultList].
///
/// In emulation mode (without `--net`), the trace is already fully processed locally, so
/// the method returns `self` as-is (or `null` if the list is empty).
///
/// Returns `null` if the chain cannot be fetched within the given attempt budget.
fun SendResultList.waitForTrace(
    self,
    quiet: bool = false,
    attempts: int = 20,
    sleepDuration: int = 1000,
): SendResultList? {
    if (self.size() == 0) {
        return null;
    }
    if (!ffi.isBroadcasting()) {
        // in emulation mode the trace is already applied locally
        return self;
    }

    val res = self.at(0);
    return ffi.waitForTrace(
        res.tx as cell,
        quiet,
        attempts,
        sleepDuration,
    );
}

/// Wait for the first transaction produced by [net.sendExternal].
fun ExternalSendResult.waitForFirstTransaction(
    self,
    quiet: bool = false,
    attempts: int = 20,
    sleepDuration: int = 1000,
): SendResult? {
    if (self.transactions == null) {
        return null;
    }
    return self.transactions!.waitForFirstTransaction(quiet, attempts, sleepDuration);
}

/// Wait for the full trace produced by [net.sendExternal].
fun ExternalSendResult.waitForTrace(
    self,
    quiet: bool = false,
    attempts: int = 20,
    sleepDuration: int = 1000,
): SendResultList? {
    if (self.transactions == null) {
        return null;
    }
    return self.transactions!.waitForTrace(quiet, attempts, sleepDuration);
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
    /// Whether it's a deploy transaction (that changes the account status from "NonExisting" to "Active").
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
    /// StateInit of the in message, encoded as Cell<StateInit?>.
    ///
    /// Exact match example:
    /// ```
    /// val expected = (init as StateInit?).toCell();
    /// expect(txs).toHaveTx({ state_init: expected });
    /// ```
    /// Predicate example:
    /// ```
    /// expect(txs).toHaveTx({
    ///     state_init: fun(rawStateInit: Cell<StateInit?>): bool {
    ///         val actual = rawStateInit.load();
    ///         // ...
    ///     },
    /// });
    /// ```
    state_init: (Cell<StateInit?> -> bool) | Cell<StateInit?> | null = null
}

/// Bind first argument to a 2-arg function, producing a 1-arg function.
fun impl.createEqFun<T>(arg: T): (T -> bool) {
    return fun(snd) {
        return arg == snd;
    };
}

fun impl.isPredicate<T>(field: (T -> bool) | T | null): bool {
    return field is (T -> bool);
}

fun impl.unwrapScalar<T>(field: (T -> bool) | T | null): T? {
    return (field is T) ? field : null;
}

/// Convert a union field to a tagged tuple for FFI:
///   [0, null] = absent
///   [1, cont] = user predicate
///   [2, cont, original_value] = value converted to predicate (original kept for display)
fun impl.toPredicate<T>(field: (T -> bool) | T | null): tuple {
    match (field) {
        (T -> bool) => {
            return [1, field];
        }
        T => {
            return [2, impl.createEqFun(field), field];
        }
        null => {
            return [0, null];
        }
    }
}

/// Check if user-provided SearchParams has any lambdas.
/// If not (most common), fast FFI path will be used.
fun SearchParams.hasPredicates(self): bool {
    return impl.isPredicate(self.to) | impl.isPredicate(self.from) | impl.isPredicate(self.value) |
    impl.isPredicate(self.exitCode) |
    impl.isPredicate(self.success) |
    impl.isPredicate(self.aborted) |
    impl.isPredicate(self.deploy) |
    impl.isPredicate(self.bounce) |
    impl.isPredicate(self.bounced) |
    impl.isPredicate(self.opcode) |
    impl.isPredicate(self.actionExitCode) |
    impl.isPredicate(self.computePhaseSkipped) |
    impl.isPredicate(self.body) |
    impl.isPredicate(self.state_init);
}

/// Convert SearchParams to a tuple of plain nullable scalar values for the fast FFI path.
fun SearchParams.toScalarFFITuple(self): tuple {
    return [
        impl.unwrapScalar(self.to),
        impl.unwrapScalar(self.from),
        impl.unwrapScalar(self.value),
        impl.unwrapScalar(self.exitCode),
        impl.unwrapScalar(self.success),
        impl.unwrapScalar(self.aborted),
        impl.unwrapScalar(self.deploy),
        impl.unwrapScalar(self.bounce),
        impl.unwrapScalar(self.bounced),
        impl.unwrapScalar(self.opcode),
        impl.unwrapScalar(self.actionExitCode),
        impl.unwrapScalar(self.computePhaseSkipped),
        impl.unwrapScalar(self.body),
        impl.unwrapScalar(self.state_init),
    ];
}

/// Convert SearchParams to a tuple of [tag, predicate] pairs for FFI.
fun SearchParams.toPredicateFFITuple(self): tuple {
    return [
        impl.toPredicate(self.to),
        impl.toPredicate(self.from),
        impl.toPredicate(self.value),
        impl.toPredicate(self.exitCode),
        impl.toPredicate(self.success),
        impl.toPredicate(self.aborted),
        impl.toPredicate(self.deploy),
        impl.toPredicate(self.bounce),
        impl.toPredicate(self.bounced),
        impl.toPredicate(self.opcode),
        impl.toPredicate(self.actionExitCode),
        impl.toPredicate(self.computePhaseSkipped),
        impl.toPredicate(self.body),
        impl.toPredicate(self.state_init),
    ];
}

/// Represents a set of parameters to search for an external-out message.
struct ExternalOutSearchParams {
    /// Source address of the external-out message.
    from: address? = null
    /// Destination address of the external-out message.
    to: any_address? = null
}

fun TlbMessage<RemainingBitsAndRefs, TlbExternalOutMessageInfo>.matches<TBody>(
    self,
    params: ExternalOutSearchParams,
): bool {
    if (params.from != null && self.info.src != params.from) {
        return false;
    }

    if (params.to != null && self.info.dest != params.to) {
        return false;
    }

    val declaredPrefix = TBody.__getDeclaredPackPrefix();
    if (declaredPrefix != -1) {
        val body = self.loadBody();
        val declaredPrefixLen = TBody.__getDeclaredPackPrefixLen();
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
/// val txs = net.send(deployer.address, message);
/// val result = txs.findExternalOutMessage({ from: counter.address });
/// expect(result).toBeNotNull();
/// val tx = result!;
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
/// If the message is not found, returns `null`.
fun SendResultList.findExternalOutMessage<TBody = unknown>(
    self,
    params: ExternalOutSearchParams = {},
): TlbMessage<TBody, TlbExternalOutMessageInfo>? {
    var transactionIdx = 0;
    repeat (self.size()) {
        val externals = self.at(transactionIdx).externals;

        var externalIdx = 0;
        repeat (externals.size()) {
            val externalOutCell = externals.get(externalIdx) as Cell<
                TlbMessage<RemainingBitsAndRefs, TlbExternalOutMessageInfo>,
            >;
            val externalOut = externalOutCell.load();

            if (externalOut.matches<TBody>(params)) {
                return {
                    info: externalOut.info,
                    init: externalOut.init,
                    body: TlbEitherLeft {
                        value: TBody.__decodeExternalOutBody(externalOut.loadBody()),
                    },
                };
            }

            externalIdx += 1;
        }

        transactionIdx += 1;
    }

    return null;
}

/// Find an external-out message in the trace produced by [net.sendExternal].
fun ExternalSendResult.findExternalOutMessage<TBody = unknown>(
    self,
    params: ExternalOutSearchParams = {},
): TlbMessage<TBody, TlbExternalOutMessageInfo>? {
    if (self.transactions == null) {
        return null;
    }
    return self.transactions!.findExternalOutMessage<TBody>(params);
}

/// Find a transaction in the send result list by specified parameters.
///
/// Example:
/// ```tolk
/// val txs = net.send(deployer.address, message);
/// val result = txs.findTransaction({ to: counter.address });
/// expect(result).toBeNotNull();
/// val tx = result!;
/// ```
fun SendResultList.findTransaction<Msg = unknown>(
    self,
    params: SearchParams = {},
): TlbTransaction? {
    val declaredPrefix = Msg.__getDeclaredPackPrefix();
    if (declaredPrefix != -1) {
        params.opcode = declaredPrefix;
    }

    val tx = impl.findTransaction(self, params);
    if (tx == null) {
        return null;
    }
    return (tx as Cell<TlbTransaction>).load();
}

/// Send a message from a specified address.
///
/// In broadcast mode (with `--net`), sending to the real network is attempted only when `from`
/// matches one of the wallets opened at script startup.
/// With `--tonconnect`, `scripts.wallet("name")` resolves to the connected TON Connect wallet
/// and matching sends are approved in that wallet instead of signed by local keys.
/// If `from` is not an opened wallet address, this method falls back to normal
/// emulator execution, even when [net.isBroadcasting] is `true`.
///
/// Example:
/// ```tolk
/// get fun `test send message`() {
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
/// In local emulation mode, returns a list of send results representing the
/// full recursively processed transaction chain produced by this send call.
///
/// Unlike [testing.processSingleTraceStep], the emulation path keeps draining
/// out messages until the whole trace queue is exhausted. Use it for normal
/// end-to-end emulation where children, grandchildren, and other follow-up
/// transactions should all be executed.
///
/// When `from` resolves to a real opened wallet and the call is routed to a
/// broadcast network instead of the local emulator, Acton sends the message to
/// that network and returns a single pseudo transaction placeholder rather than
/// recursively executing descendants locally.
///
/// See [testing.processSingleTraceStep] if you need to send a single message without out message processing.
fun net.send(from: address, message: OutMessage): SendResultList {
    val rawRes = ffi.sendMessageFrom(message.messageCell, from.toCell());
    if (rawRes == null) {
        // just throw to show original error message from Rust
        throw 100;
    }
    return rawRes;
}

/// The response from [net.createExternalMessage].
/// It stores an already composed message cell and the destination address used
/// by [net.sendExternal] diagnostics.
struct ExternalInMessage {
    /// Encoded external-in message cell.
    messageCell: cell
    /// Destination account address.
    dest: address
}

/// Creates an external-in message.
/// Body is always packed into the Either Right (in the reference).
/// If `StateInit` is present it is also packed in the Either Right (in the reference).
/// Full TL-B scheme of the message is.
/// ```
/// ext_in_msg_info$10 src:MsgAddressExt dest:MsgAddressInt
///   import_fee:Grams = CommonMsgInfo;
/// message$_ {X:Type} info:CommonMsgInfo
///   init:(Maybe (Either StateInit ^StateInit))
///   body:(Either X ^X) = Message X;
/// ```
fun net.createExternalMessage<TBody>(
    dest: address,
    body: TBody,
    stateInit: StateInit? = null,
    src: any_address? = null,
): ExternalInMessage {
    val init: TlbMaybe<TlbEither<StateInit, Cell<StateInit>>> = stateInit != null
        ? TlbMaybe.just(TlbEither.right(stateInit.toCell()))
        : TlbMaybe.none();

    // A little hack because Tolk does not support arbitrary function calls as default parameters
    if (src == null) {
        src = createAddressNone();
    }
    // Tolk does not have ext_address type, so ext_address = any_address \ int_address
    if (src.isInternal()) {
        Assert.fail("Internal source address is not supported for External-in messages");
    }

    return {
        messageCell: TlbMessage<TBody, TlbExternalInMessageInfo> {
            info: TlbExternalInMessageInfo {
                src,
                dest,
                // It is better to fairly calculate the import fee right here or in acton
                importFee: ton("0.1"),
            },
            init,
            body: TlbEither<TBody, Cell<TBody>>.right(body.__asPayload()),
        }.toCell(),
        dest,
    };
}

/// Sends an external-in message to the specified destination address.
/// Read more about external incoming messages: https://docs.ton.org/foundations/messages/external-in#incoming-external-messages
///
/// In local emulation, this helper recursively processes the resulting trace just
/// like [net.send], but it starts from an already composed external-in
/// message instead of an internal message sent from a wallet-like address.
///
/// In broadcast mode without `--tonconnect`, Acton sends the external-in message to
/// the network and returns a single pseudo transaction placeholder. With
/// `--tonconnect`, `net.sendExternal` is rejected; use [net.send] with an
/// internal wallet message so the connected wallet can approve it.
///
/// If the external message is accepted, the returned [ExternalSendResult]
/// contains the full transaction trace in `transactions`.
///
/// If the external message is not accepted, the destination account does not
/// exist, or emulation fails before a trace is produced, the returned result has
/// `transactions = null` and `error` contains all available emulator metadata.
/// In tests, prefer `expect(result).toBeAccepted()` or
/// [ExternalSendResult.unwrap] so Acton can render the external status, VM exit
/// code, ABI error name, source location, and backtrace.
///
/// Example:
/// ```tolk
/// val result = net.sendExternal(msg);
/// expect(result).toBeAccepted();
///
/// val txs = result.unwrap();
/// expect(txs).toHaveAllSuccessfulTxs();
/// ```
fun net.sendExternal(msg: ExternalInMessage): ExternalSendResult {
    val result = ffi.sendExternalMessage(msg.messageCell);
    val error = result.get(1);
    return ExternalSendResult {
        transactions: result.get(0) as SendResultList?,
        error: error == null ? null : error as ExternalSendError,
        destination: msg.dest,
    };
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
/// This is useful for testing single messages with [testing.processSingleTraceStep].
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
fun OutMessage.bounced(self): OutMessage {
    var msg = (self.messageCell as Cell<TlbMessageRelaxedGeneric>).load();
    msg.info.bounced = true;
    return { messageCell: msg.toCell() };
}

/// Returns the current runtime broadcast flag.
///
/// The flag is normally enabled by running a script with `--net`, and can also be
/// changed at runtime with [net.enableBroadcast] and [net.disableBroadcast].
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
/// Wallets are loaded once at startup (when the script is launched with CLI `--net`).
/// Calling [net.enableBroadcast] later does not load wallets, so [scripts.wallet] can still fail
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

/// Run a get method of a contract by its name or id.
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
/// val address = net.runGetMethod<address>(wallet.address, "get_wallet_address", [address("...")]);
/// ```
///
/// If the get method accepts several arguments, you can pass them as a third argument as an array.
/// ```tolk
/// get fun do_smth(owner: address, id: int): address { ... }
/// ```
/// ```tolk
/// val address = net.runGetMethod<address>(wallet.address, "do_smth", [deployer.address, 10]);
/// ```
///
/// ## Specify the method's return type as a type parameter
///
/// Specify the return type as a generic parameter:
/// ```tolk
/// get fun get_jetton_data(): JettonDataReply { ... }
/// ```
/// ```tolk
/// val data = net.runGetMethod<JettonDataReply>(wallet.address, "get_jetton_data");
/// ```
fun net.runGetMethod<Ret>(addr: address, idOrName: int | string, args: array<unknown> = []): Ret {
    val (methodId, methodName) = match (idOrName) {
        int => (idOrName, ""),
        string => ((ffi.crc16(idOrName) & 0xffff) | 0x10000, idOrName),
    };
    assert (ffi.isDeployed(addr.toCell())) throw CANNOT_RUN_GET_METHOD_OF_UNDEPLOYED_CONTRACT;

    val code = ffi.getDeployedCode(addr.toCell());
    assert (code != null) throw CANNOT_RUN_GET_METHOD_OF_CONTRACT_WITHOUT_CODE;

    val res = ffi.runGetMethod(
        addr.toCell(),
        code,
        methodId,
        methodName,
        args,
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
                format(
                    "Get method '{}' returned '{}' that cannot be deserialized to '{}'. Make sure you passed all parameters to the get method.",
                    methodName,
                    res,
                    reflect.typeNameOf<Ret>(),
                ),
            );
        }

        throw e;
    }
}

/// Generate a random address.
///
/// If a name is provided, the address will be registered with the given name.
/// This allows test runner to show the name instead of the address in the test output.
///
/// Example:
/// ```tolk
/// val address = randomAddress("random_user");
/// val masterchainAddress = randomAddress("random_user", -1);
/// ```
fun randomAddress(name: string? = null, wc: int = BASECHAIN): address {
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
        repeat (32) {
            val num = random.range(256);
            builder.storeUint(num, 8);
        }
    }

    val address = address.fromValidBuilder(builder);
    if (name != null) {
        ffi.registerAddress(address.toCell(), name);
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

/// Load library by hash, first from the current emulated blockchain and then from the configured network.
///
/// In forked tests this means locally registered libraries win over the remote
/// fork network for the same hash.
///
/// Returns null if library is not found.
fun net.loadLibrary(hash: string): cell? {
    return ffi.loadLibrary(hash);
}

fun tuple.fromTuple(data: tuple): tuple {
    return data;
}

fun T.__decodeExternalOutBody(body: RemainingBitsAndRefs): T {
    return T.fromSlice(body);
}

fun unknown.__decodeExternalOutBody(_: RemainingBitsAndRefs): unknown {
    return null;
}

// This function is used in createExternalMessage to not wrap cells twice
fun T.__asPayload(self): Cell<T> {
    return self.toCell();
}

fun Cell<T>.__asPayload(self): Cell<T> {
    return self;
}

fun cell.__asPayload(self): Cell<cell> {
    return self as Cell<cell>;
}

/// Expect the external-in message to be accepted and produce a transaction trace.
///
/// On failure, Acton reports the external status, emulator reason, VM exit code
/// when available, a known exit-code description or ABI error name, source
/// location, and the full backtrace when the test is run with `--backtrace full`.
fun Expectation<ExternalSendResult>.toBeAccepted(self): void {
    if (self.value.transactions != null) {
        return;
    }
    self.value.__failExternalSend("expect(<actual>).toBeAccepted()", self.location);
}

/// Expect the external-in message to be rejected before acceptance.
fun Expectation<ExternalSendResult>.toBeNotAccepted(self): void {
    if (self.value.error != null && self.value.error.externalNotAccepted) {
        return;
    }
    if (self.value.error != null) {
        self.fail(
            format(
                "expect(<actual>).toBeNotAccepted(): external send failed, but not as an external-not-accepted error: {}",
                self.value.error.message,
            ),
        );
    }
    self.fail("expect(<actual>).toBeNotAccepted(): external message was accepted");
}

/// Expect the external send error to have a specific VM exit code.
///
/// This matcher is intended for rejected external-in messages. For accepted
/// external messages, use transaction matchers on the produced [SendResultList].
fun Expectation<ExternalSendResult>.toEndWithExitCode(self, exitCode: int): void {
    if (
        self.value.error != null && self.value.error.vmExitCode != null &&
        self.value.error.vmExitCode == exitCode
    ) {
        return;
    }
    if (self.value.error != null) {
        self.fail(
            format(
                "expect(<actual>).toEndWithExitCode(<expected>): expected {}, got {}",
                exitCode,
                self.value.error.vmExitCode,
            ),
        );
    }
    self.fail(
        format(
            "expect(<actual>).toEndWithExitCode(<expected>): expected {}, but external message was accepted",
            exitCode,
        ),
    );
}

/// Expect the transaction list to have a successful deploy transaction.
///
/// Transaction is considered as "deploy" if previous status was "NonExisting" and end status was "Active".
/// If generic `Msg` is provided and has declared prefix, matcher also filters by `params.opcode`.
///
/// Example:
/// ```tolk
/// expect(txs).toHaveSuccessfulDeploy({
///     to: counter.address,
/// });
/// ```
fun Expectation<SendResultList>.toHaveSuccessfulDeploy<Msg = unknown>(
    self,
    params: SearchParams = {},
): void {
    val declaredPrefix = Msg.__getDeclaredPackPrefix();
    if (declaredPrefix != -1) {
        params.opcode = declaredPrefix;
    }
    params.success = true;
    params.deploy = true;

    impl.findTransactionOrFail(
        self.value,
        params,
        "expect(<actual>).toHaveSuccessfulDeploy(<expected>)",
        self.location,
    );
}

/// Expect the transaction list to have a successful transaction.
///
/// Matcher sets `params.success = true`.
/// Runtime "success" means both compute phase and action phase are successful.
///
/// By default matcher also requires `params.exitCode = 0` when `exitCode` is not provided.
/// This default can be overridden with explicit `params.exitCode`.
///
/// If generic `Msg` is provided and has declared prefix, matcher also filters by `params.opcode`.
///
/// Example:
/// ```tolk
/// expect(txs).toHaveSuccessfulTx({
///     from: deployer.address,
///     to: counter.address,
/// });
/// ```
fun Expectation<SendResultList>.toHaveSuccessfulTx<Msg = unknown>(
    self,
    params: SearchParams = {},
): void {
    val declaredPrefix = Msg.__getDeclaredPackPrefix();
    if (declaredPrefix != -1) {
        params.opcode = declaredPrefix;
    }
    params.success = true;
    if (params.exitCode == null) {
        params.exitCode = 0;
    }

    impl.findTransactionOrFail(
        self.value,
        params,
        "expect(<actual>).toHaveSuccessfulTx(<expected>)",
        self.location,
    );
}

/// Expect the transaction list to have a transaction with specified parameters.
///
/// All non-null `SearchParams` fields are applied as filters.
///
/// If generic `Msg` is provided and has declared prefix, matcher also filters by `params.opcode`.
///
/// Example:
/// ```tolk
/// expect(txs).toHaveTx({
///     from: deployer.address,
///     to: counter.address,
///     actionExitCode: 37,
/// });
/// ```
fun Expectation<SendResultList>.toHaveTx<Msg = unknown>(self, params: SearchParams = {}): void {
    val declaredPrefix = Msg.__getDeclaredPackPrefix();
    if (declaredPrefix != -1) {
        params.opcode = declaredPrefix;
    }

    impl.findTransactionOrFail(
        self.value,
        params,
        "expect(<actual>).toHaveTx(<expected>)",
        self.location,
    );
}

/// Expect the transaction list to NOT have a transaction with specified parameters.
///
/// Example:
/// ```tolk
/// expect(txs).toNotHaveTx({
///     from: deployer.address,
///     to: counter.address,
/// });
/// ```
fun Expectation<SendResultList>.toNotHaveTx<Msg = unknown>(self, params: SearchParams = {}): void {
    val declaredPrefix = Msg.__getDeclaredPackPrefix();
    if (declaredPrefix != -1) {
        params.opcode = declaredPrefix;
    }

    impl.findNoTransactionOrFail(
        self.value,
        params,
        "expect(<actual>).toNotHaveTx(<expected>)",
        self.location,
    );
}

/// Expect the transaction list to have a **bounced** transaction.
///
/// Example:
/// ```tolk
/// expect(txs).toHaveBouncedTx({
///     from: deployer.address,
///     to: counter.address,
/// });
/// ```
fun Expectation<SendResultList>.toHaveBouncedTx<Msg = unknown>(
    self,
    params: SearchParams = {},
): void {
    val declaredPrefix = Msg.__getDeclaredPackPrefix();
    if (declaredPrefix != -1) {
        params.opcode = declaredPrefix;
    }
    params.bounced = true;

    impl.findTransactionOrFail(
        self.value,
        params,
        "expect(<actual>).toHaveBouncedTx(<expected>)",
        self.location,
    );
}

/// Expect the transaction list to have a **failed** transaction.
///
/// This matcher sets `params.success = false`.
/// `params.exitCode` is required and must be non-zero.
/// If `exitCode` is null or `0`, matcher fails immediately before transaction search.
///
/// Any additional non-null `SearchParams` fields further narrow the search.
/// If generic `Msg` is provided and has declared prefix, matcher also filters by `params.opcode`.
///
/// Example:
/// ```tolk
/// expect(txs).toHaveFailedTx({
///     from: deployer.address,
///     to: counter.address,
///     exitCode: 123,
/// });
/// ```
fun Expectation<SendResultList>.toHaveFailedTx<Msg = unknown>(
    self,
    params: SearchParams = {},
): void {
    val declaredPrefix = Msg.__getDeclaredPackPrefix();
    if (declaredPrefix != -1) {
        params.opcode = declaredPrefix;
    }
    params.success = false;

    val nonZeroExitCode = params.exitCode is int32 && params.exitCode as int32 != 0;
    if (!nonZeroExitCode) {
        self.fail("expect(<actual>).toHaveFailedTx(params) must have non zero exitCode parameter in params");
    }

    impl.findTransactionOrFail(
        self.value,
        params,
        "expect(<actual>).toHaveFailedTx(<expected>)",
        self.location,
    );
}

/// Expect all transactions in the list to be successful.
///
/// Example:
/// ```tolk
/// val txs = net.send(wallet.address, msg);
/// expect(txs).toHaveAllSuccessfulTxs();
/// ```
fun Expectation<SendResultList>.toHaveAllSuccessfulTxs(self): void {
    val params = SearchParams { success: false };
    impl.findNoTransactionOrFail(
        self.value,
        params,
        "expect(<actual>).toHaveAllSuccessfulTxs(<expected>)",
        self.location,
    );
}

/// Expect the transaction to emit an external message of given type.
///
/// Example:
/// ```tolk
/// expect(txs).toEmitExternalMessage<IncreaseCounter>();
/// ```
fun Expectation<SendResultList>.toEmitExternalMessage<Msg>(self): void {
    if (self.value.findExternalOutMessage<Msg>() != null) {
        return;
    }

    ffi.failToFindExternalMessage(
        self.location,
        format(
            "expect(<actual>).toEmitExternalMessage<{}>()",
            reflect.typeNameOf<Msg>(),
        ),
        self.value,
        reflect.typeNameOf<Msg>(),
        reflect.serializationPrefixOf<Msg>().0,
    );
    throw ASSERTION_FAILED;
}

/// Expect the transaction to consume less than the expected amount of gas.
///
/// Example:
/// ```tolk
/// val txs = net.send(deployer.address, message);
/// expect(txs.at(0)).toConsumeLessThan(1000);
/// ```
fun Expectation<SendResult>.toConsumeLessThan(self, amount: int): void {
    ffi.assertComparison(
        "<",
        self.value.gasUsed,
        amount,
        "expect(<actual>).toConsumeLessThan(<expected>)",
        self.location,
    );
}

fun T.__getDeclaredPackPrefix(): int {
    return reflect.serializationPrefixOf<T>().0;
}

fun unknown.__getDeclaredPackPrefix(): int {
    return -1;
}

fun T.__getDeclaredPackPrefixLen(): int {
    return reflect.serializationPrefixOf<T>().1;
}

fun unknown.__getDeclaredPackPrefixLen(): int {
    return 0;
}

fun impl.findTransaction(txs: SendResultList, params: SearchParams): cell? {
    if (txs.size() == 0) {
        return null;
    }

    return params.hasPredicates()
        ? ffi.findTransactionByPredicateParams(
            txs.first().lt,
            txs.last().lt,
            params.toPredicateFFITuple(),
        )
        : ffi.findTransactionByScalarParams(
            txs.first().lt,
            txs.last().lt,
            params.toScalarFFITuple(),
        );
}

fun impl.findTransactionOrFail(
    txs: SendResultList,
    params: SearchParams,
    message: string,
    location: string,
): void {
    val tx = impl.findTransaction(txs, params);
    if (tx == null) {
        ffi.failToFindTransactionByParams(location, message, txs, params.toPredicateFFITuple());
        throw ASSERTION_FAILED;
    }
}

fun impl.findNoTransactionOrFail(
    txs: SendResultList,
    params: SearchParams,
    message: string,
    location: string,
): void {
    val tx = impl.findTransaction(txs, params);
    if (tx != null) {
        ffi.failToNotFindTransactionByParams(location, message, txs, params.toPredicateFFITuple());
        throw ASSERTION_FAILED;
    }
}

fun ffi.sendMessageFrom(msg: cell, from: cell): SendResultList?
    asm "9 EXTCALL"

fun ffi.sendExternalMessage(msg: cell): tuple
    asm "56 EXTCALL"

fun ffi.runGetMethod(
    addr: cell,
    code: cell,
    methodId: int,
    methodName: string,
    args: tuple,
): tuple?
    asm "8 EXTCALL"

fun ffi.isDeployed(addr: cell): bool
    asm "11 EXTCALL"

fun ffi.getDeployedCode(addr: cell): cell?
    asm "12 EXTCALL"

fun ffi.registerAddress(address: cell, name: string): void
    asm "15 EXTCALL"

fun ffi.registerCodeCell(cell: cell, name: string): void
    asm "16 EXTCALL"

fun ffi.findTransactionByScalarParams(firstTxLt: int, lastTxLt: int, params: tuple): cell?
    asm "10 EXTCALL"

fun ffi.findTransactionByPredicateParams(firstTxLt: int, lastTxLt: int, params: tuple): cell?
    asm "48 EXTCALL"

fun ffi.failToFindTransactionByParams(
    location: string,
    message: string,
    txs: SendResultList,
    params: tuple,
): tuple?
    asm "103 EXTCALL"

fun ffi.failToNotFindTransactionByParams(
    location: string,
    message: string,
    txs: SendResultList,
    params: tuple,
): tuple?
    asm "104 EXTCALL"

fun ffi.failToFindExternalMessage(
    location: string,
    message: string,
    txs: SendResultList,
    messageName: string,
    opcode: int,
): tuple?
    asm "108 EXTCALL"

fun ffi.failExternalSendNotAccepted(
    location: string,
    message: string,
    error: tuple,
): tuple?
    asm "109 EXTCALL"

fun ffi.loadLibrary(hash: string): cell?
    asm "21 EXTCALL"

fun ffi.isBroadcasting(): bool
    asm "23 EXTCALL"

fun ffi.waitForTransaction(
    pseudoTxCell: cell,
    quiet: bool,
    attempts: int,
    sleepDuration: int,
): SendResultList?
    asm "25 EXTCALL"

fun ffi.waitForTrace(
    pseudoTxCell: cell,
    quiet: bool,
    attempts: int,
    sleepDuration: int,
): SendResultList?
    asm "49 EXTCALL"

fun ffi.enableBroadcast(): void
    asm "26 EXTCALL"

fun ffi.disableBroadcast(): void
    asm "27 EXTCALL"

fun ffi.getConfig(): dict
    asm "31 EXTCALL"

fun ffi.saveTraceName(txs: SendResultList, name: string): void
    asm "35 EXTCALL"

fun ffi.crc16(data: string): int
    asm "13 EXTCALL"
