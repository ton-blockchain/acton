/// Module defining transaction types.
///
/// This module provides a complete representation of TON transactions as they appear in the blockchain.
/// It includes structures for all phases of transaction execution: storage, credit, compute, action, and bounce.
///
/// Examples:
/// ```tolk
/// val tx = results.at(0).tx.load();
///
/// // Check gas usage
/// val gas = tx.getUsedGas();
/// println("Gas used: {}", gas);
///
/// // Inspect account status change
/// val description = tx.description.load();
/// if (description is TlbTransOrd) {
///     val compute = description.computePh;
///     if (compute is TlbTrComputeVm) {
///         println("VM exit code: {}", compute.exitCode);
///     }
/// }
/// ```

import "message"
import "../impl"
import "../testing/assert"
import "../tlb/maybe"

/// Type alias for transaction account status values used by [TlbTransaction.origStatus] and
/// [TlbTransaction.endStatus].
enum TlbAccountStatus: uint2 {
    Uninitialized,
    Frozen,
    Active,
    NonExisting,
}

/// Represents depth and balance information for split accounts.
struct TlbDepthBalanceInfo {
    splitDepth: uint8 // #<= 30
    balance: TlbCurrencyCollection
}

/// Represents a transaction in the TON blockchain.
struct (0b0111) TlbTransaction {
    /// Account address as 256-bit hash.
    accountAddr: bits256
    /// Logical time of the transaction.
    lt: uint64
    /// Hash of the previous transaction.
    prevTransHash: bits256
    /// Logical time of the previous transaction.
    prevTransLt: uint64
    /// Current time (Unix timestamp).
    now: uint32
    /// Number of outgoing messages.
    outmsgCnt: uint15
    /// Account status before transaction.
    origStatus: TlbAccountStatus
    /// Account status after transaction.
    endStatus: TlbAccountStatus
    /// In and out messages.
    messages: Cell<TlbTransactionMessages>
    /// Total fees collected.
    totalFees: TlbCurrencyCollection
    /// State update information.
    stateUpdate: Cell<TlbHashUpdate<TlbAccount>>
    /// Transaction description.
    description: Cell<TlbTransactionDescr>
}

/// Loads the in message body of the transaction with given type.
///
/// Fails if transaction has no inbound message (`messages.inMsg` is `TlbNone`), because this
/// method unwraps it before typed decoding.
///
/// After an inbound message exists, if message type does not match `Msg`, loading fails with
/// exit code 63. Internal and external-in transaction bodies are both supported.
///
/// Example:
/// ```tolk
/// val txs = net.send(address, msg);
/// val tx = txs.findTransaction<IncreaseCounter>({ to: address })!;
/// val body = tx.loadBody<IncreaseCounter>();
/// println(body.increaseBy);
/// ```
fun TlbTransaction.loadBody<Msg>(self): Msg {
    val inMsgCell = self.messages.load().inMsg.unwrap() as cell as Cell<TlbMessage<Msg>>;
    val inMsg = inMsgCell.load();
    return inMsg.loadBody();
}

/// Loads the in message of the transaction with given type.
///
/// Fails if transaction has no inbound message (`messages.inMsg` is `TlbNone`), because this
/// method unwraps it before typed decoding.
///
/// After an inbound message exists, if message type does not match `Msg`, loading fails with
/// exit code 63. Internal and external-in transaction messages are both supported.
fun TlbTransaction.loadInMsg<Msg>(self): TlbMessage<Msg> {
    val inMsgCell = self.messages.load().inMsg.unwrap() as cell as Cell<TlbMessage<Msg>>;
    return inMsgCell.load();
}

/// Returns consumed gas for ordinary transactions with VM compute phase.
///
/// Fails with [Assert.fail]:
/// - if `description` is not [TlbTransOrd];
/// - if `description.computePh` is [TlbTrComputeSkipped].
fun TlbTransaction.getUsedGas(self): int {
    val description = self.description.load();
    if (description is TlbTransOrd) {
        if (description.computePh is TlbTrComputeVm) {
            return description.computePh.data.load().gasUsed as int;
        } else {
            Assert.fail("Cannot get used gas for transaction with skipped compute phase");
        }
    } else {
        Assert.fail("Cannot get used gas for non TransOrd");
    }
}

/// Returns action-phase fee based on [TlbTransaction.description] variant.
///
/// Behavior by variant:
/// - [TlbTransOrd] and [TlbTransTickTock] with present `action`: returns `action.totalActionFees.unwrapOr(null)`;
/// - [TlbTransOrd] and [TlbTransTickTock] with `action = TlbNone`: returns `null`;
/// - all other [TlbTransactionDescr] variants: returns `null`.
fun TlbTransaction.getActionFee(self): coins? {
    val descr = self.description.load();
    if (descr is TlbTransOrd) {
        val action = descr.action;
        if (action is TlbNone) {
            return null;
        }
        val actionPh = action.value.load();
        return actionPh.totalActionFees.unwrapOr(null);
    }
    if (descr is TlbTransTickTock) {
        val action = descr.action;
        if (action is TlbNone) {
            return null;
        }
        val actionPh = action.value.load();
        return actionPh.totalActionFees.unwrapOr(null);
    }
    return null;
}

/// Return address of account of this transaction.
fun TlbTransaction.getAccountAddress(self, workchain: int = BASECHAIN): address {
    return address.fromValidBuilder(
        beginCell()
            .storeUint(0b100, 3)
            .storeInt(workchain, 8)
            .storeSlice(self.accountAddr as slice),
    );
}

struct TlbTransactionMessages {
    /// In message.
    inMsg: TlbMaybe<Cell<TlbMessageRelaxedGeneric>>
    /// Out messages.
    outMsgs: map<uint15, Cell<TlbMessageRelaxedGeneric>>
}

/// Represents merkle update for proof-of-work.
struct (0b0100) TlbMerkleUpdate<X> {
    /// Old hash.
    oldHash: bits256
    /// New hash.
    newHash: bits256
    /// Old depth.
    oldDepth: uint16
    /// New depth.
    newDepth: uint16
    /// Old value.
    old: X
    /// New value.
    new: X
}

/// Represents simple hash update.
struct (0b01110010) TlbHashUpdate<X> {
    /// Old hash.
    oldHash: bits256
    /// New hash.
    newHash: bits256
}

/// Represents merkle proof.
struct (0b0011) TlbMerkleProof<X> {
    /// Virtual hash.
    virtualHash: bits256
    /// Depth.
    depth: uint16
    /// Virtual root.
    virtualRoot: X
}

/// Union type for hash updates.
type TlbHashUpdateType<X> = TlbMerkleUpdate<X> | TlbHashUpdate<X> | TlbMerkleProof<X>

/// Represents storage phase of transaction processing.
struct TlbTrStoragePhase {
    /// Storage fees collected.
    storageFeesCollected: coins
    /// Storage fees due (if any).
    storageFeesDue: TlbMaybe<coins>
    /// Account status change.
    statusChange: TlbAccStatusChange
}

struct (0b0) TlbAccUnchanged
struct (0b10) TlbAccFrozen
struct (0b11) TlbAccDeleted

type TlbAccStatusChange = TlbAccUnchanged | TlbAccFrozen | TlbAccDeleted

/// Represents credit phase of transaction processing.
struct TlbTrCreditPhase {
    /// Due fees collected (if any).
    dueFeesCollected: TlbMaybe<coins>
    /// Credit amount.
    credit: TlbCurrencyCollection
}

/// Represents compute phase of transaction processing.
struct (0b0) TlbTrComputeSkipped {
    reason: TlbComputeSkipReason
}

struct (0b1) TlbTrComputeVm {
    success: bool
    msgStateUsed: bool
    accountActivated: bool
    gasFees: coins
    data: Cell<TlbTrComputeVmRef>
}

struct TlbTrComputeVmRef {
    gasUsed: TlbVarUint7
    gasLimit: TlbVarUint7
    gasCredit: TlbMaybe<TlbVarUint3>
    mode: int8
    exitCode: int32
    exitArg: TlbMaybe<int32>
    vmSteps: uint32
    vmInitStateHash: bits256
    vmFinalStateHash: bits256
}

/// Union type for compute phases.
type TlbTrComputePhase = TlbTrComputeSkipped | TlbTrComputeVm

/// Represents reasons for skipping computation.
struct (0b00) TlbCSkipNoState
struct (0b01) TlbCSkipBadState
struct (0b10) TlbCSkipNoGas
struct (0b110) TlbCSkipSuspended

type TlbComputeSkipReason = TlbCSkipNoState | TlbCSkipBadState | TlbCSkipNoGas | TlbCSkipSuspended

/// Represents action phase of transaction processing.
struct TlbTrActionPhase {
    success: bool
    valid: bool
    noFunds: bool
    statusChange: TlbAccStatusChange
    totalFwdFees: TlbMaybe<coins>
    totalActionFees: TlbMaybe<coins>
    resultCode: int32
    resultArg: TlbMaybe<int32>
    totActions: uint16
    specActions: uint16
    skippedActions: uint16
    msgsCreated: uint16
    actionListHash: bits256
    totMsgSize: TlbStorageUsed
}

/// Represents bounce phase of transaction processing.
struct (0b00) TlbTrBounceNegfunds

struct (0b01) TlbTrBounceNofunds {
    msgSize: TlbStorageUsed
    reqFwdFees: coins
}

struct (0b1) TlbTrBounceOk {
    msgSize: TlbStorageUsed
    msgFees: coins
    fwdFees: coins
}

/// Union type for bounce phases.
type TlbTrBouncePhase = TlbTrBounceNegfunds | TlbTrBounceNofunds | TlbTrBounceOk

/// Represents ordinary transaction description.
@overflow1023_policy("suppress")
struct (0b0000) TlbTransOrd {
    creditFirst: bool
    storagePh: TlbMaybe<TlbTrStoragePhase>
    creditPh: TlbMaybe<TlbTrCreditPhase>
    computePh: TlbTrComputePhase
    action: TlbMaybe<Cell<TlbTrActionPhase>>
    aborted: bool
    bounce: TlbMaybe<TlbTrBouncePhase>
    destroyed: bool
}

/// Represents storage transaction description.
struct (0b0001) TlbTransStorage {
    storagePh: TlbTrStoragePhase
}

/// Represents tick-tock transaction description.
@overflow1023_policy("suppress")
struct (0b001) TlbTransTickTock {
    isTock: bool
    storagePh: TlbTrStoragePhase
    computePh: TlbTrComputePhase
    action: TlbMaybe<Cell<TlbTrActionPhase>>
    aborted: bool
    destroyed: bool
}

/// Represents split-merge information.
struct TlbSplitMergeInfo {
    curShardPfxLen: uint8 // ## 6
    accSplitDepth: uint8  // ## 6
    thisAddr: bits256
    siblingAddr: bits256
}

/// Represents split prepare transaction description.
@overflow1023_policy("suppress")
struct (0b0100) TlbTransSplitPrepare {
    splitInfo: TlbSplitMergeInfo
    storagePh: TlbMaybe<TlbTrStoragePhase>
    computePh: TlbTrComputePhase
    action: TlbMaybe<Cell<TlbTrActionPhase>>
    aborted: bool
    destroyed: bool
}

/// Represents split install transaction description.
struct (0b0101) TlbTransSplitInstall {
    splitInfo: TlbSplitMergeInfo
    prepareTransaction: Cell<TlbTransaction>
    installed: bool
}

/// Represents merge prepare transaction description.
struct (0b0110) TlbTransMergePrepare {
    splitInfo: TlbSplitMergeInfo
    storagePh: TlbTrStoragePhase
    aborted: bool
}

/// Represents merge install transaction description.
@overflow1023_policy("suppress")
struct (0b0111) TlbTransMergeInstall {
    splitInfo: TlbSplitMergeInfo
    prepareTransaction: Cell<TlbTransaction>
    storagePh: TlbMaybe<TlbTrStoragePhase>
    creditPh: TlbMaybe<TlbTrCreditPhase>
    computePh: TlbTrComputePhase
    action: TlbMaybe<Cell<TlbTrActionPhase>>
    aborted: bool
    destroyed: bool
}

/// Union type for all transaction descriptions.
type TlbTransactionDescr =
    | TlbTransOrd
    | TlbTransStorage
    | TlbTransTickTock
    | TlbTransSplitPrepare
    | TlbTransSplitInstall
    | TlbTransMergePrepare
    | TlbTransMergeInstall

type TlbVarUint7 = int

fun impl.varUintSize(value: int, maxBytes: int): int {
    if (value < 0) {
        Assert.fail("VarUInteger value must be non-negative");
    }
    if (value == 0) {
        return 0;
    }

    var size = 1;
    var threshold = 256;
    while (size < maxBytes && value >= threshold) {
        size += 1;
        threshold *= 256;
    }

    if (value >= threshold) {
        Assert.fail("VarUInteger value is out of range");
    }

    return size;
}

fun TlbVarUint7.packToBuilder(self, mutate b: builder): void {
    val size = impl.varUintSize(self, 7);
    b.storeUint(size, 3);
    if (size > 0) {
        b.storeUint(self, size * 8);
    }
}

fun TlbVarUint7.unpackFromSlice(mutate s: slice): TlbVarUint7 {
    val size = s.loadUint(3);
    return s.loadUint(size * 8);
}

type TlbVarUint3 = int

fun TlbVarUint3.packToBuilder(self, mutate b: builder): void {
    val size = impl.varUintSize(self, 3);
    b.storeUint(size, 2);
    if (size > 0) {
        b.storeUint(self, size * 8);
    }
}

fun TlbVarUint3.unpackFromSlice(mutate s: slice): TlbVarUint3 {
    val size = s.loadUint(2);
    return s.loadUint(size * 8);
}

/// Represents storage used information.
struct TlbStorageUsed {
    cells: TlbVarUint7
    bits: TlbVarUint7
}

/// Represents storage extra information.
struct (0b000) TlbStorageExtraNone

struct (0b001) TlbStorageExtraDict {
    dictHash: uint256
}

/// Union type for storage extra information.
type TlbStorageExtraInfo = TlbStorageExtraNone | TlbStorageExtraDict

/// Represents storage information.
struct TlbStorageInfo {
    /// Storage used.
    used: TlbStorageUsed
    /// Storage extra information.
    storageExtra: TlbStorageExtraInfo
    /// Last paid time.
    lastPaid: uint32
    /// Due payment (optional).
    duePayment: TlbMaybe<coins>
}

/// Represents account.
struct (0b0) TlbAccountNone

struct (0b1) TlbAccountInfo {
    /// Account address.
    addr: address
    /// Storage statistics.
    storageStat: TlbStorageInfo
    /// Account storage.
    storage: TlbAccountStorage
}

/// Union type for account.
type TlbAccount = TlbAccountNone | TlbAccountInfo

/// Account state in [TlbAccountStorage] (block.tlb `AccountState`).
struct (0b00) TlbAccountStateUninit

struct (0b1) TlbAccountStateActive {
    stateInit: StateInit
}

struct (0b01) TlbAccountStateFrozen {
    stateHash: uint256
}

/// Union type for account storage state.
type TlbAccountState = TlbAccountStateUninit | TlbAccountStateActive | TlbAccountStateFrozen

/// Represents account storage.
struct TlbAccountStorage {
    /// Last transaction logical time.
    lastTransLt: uint64
    /// Account balance.
    balance: TlbCurrencyCollection
    /// Account state.
    state: TlbAccountState
}

/// Represents a shard account state.
struct TlbShardAccount {
    account: Cell<TlbAccount>
    lastTransHash: uint256
    lastTransLt: uint64
}
