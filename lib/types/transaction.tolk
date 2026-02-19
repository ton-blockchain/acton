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
/// println1("Gas used: {}", gas);
///
/// // Inspect account status change
/// val description = tx.description.load();
/// if (description is TransOrd) {
///     val compute = description.computePh;
///     if (compute is TrComputeVm) {
///         println1("VM exit code: {}", compute.exitCode);
///     }
/// }
/// ```

import "message"
import "../testing/assert"
import "../tlb/maybe"

struct (0b00) Uninitialized
struct (0b01) Frozen
struct (0b10) Active
struct (0b11) NonExisting

/// Type alias for transaction account status values used by [Transaction.origStatus] and
/// [Transaction.endStatus].
type AccountStatus = Uninitialized | Frozen | Active | NonExisting

/// Represents depth and balance information for split accounts.
struct DepthBalanceInfo {
    splitDepth: uint8, // #<= 30
    balance: CurrencyCollection
}

/// Represents a transaction in the TON blockchain.
struct (0b0111) Transaction {
    /// Account address as 256-bit hash.
    accountAddr: bits256,
    /// Logical time of the transaction.
    lt: uint64,
    /// Hash of the previous transaction.
    prevTransHash: bits256,
    /// Logical time of the previous transaction.
    prevTransLt: uint64,
    /// Current time (Unix timestamp).
    now: uint32,
    /// Number of outgoing messages.
    outmsgCnt: uint15,
    /// Account status before transaction.
    origStatus: AccountStatus,
    /// Account status after transaction.
    endStatus: AccountStatus,
    /// In and out messages.
    messages: Cell<TransactionMessages>,
    /// Total fees collected.
    totalFees: CurrencyCollection,
    /// State update information.
    stateUpdate: Cell<HashUpdate<Account>>,
    /// Transaction description.
    description: Cell<TransactionDescr>
}

/// Loads the in message body of the transaction with given type.
///
/// Fails if transaction has no inbound message (`messages.inMsg` is `None`), because this
/// method unwraps it before typed decoding.
///
/// After an inbound message exists, if message type does not match `Msg`, loading fails with
/// exit code 63.
///
/// Example:
/// ```tolk
/// val result = net.send(address, msg);
/// val tx = result.findTransaction<IncreaseCounter>({ to: address }).unwrap();
/// val body = tx.loadBody<IncreaseCounter>();
/// println(body.increaseBy);
/// ```
fun Transaction.loadBody<Msg>(self) {
    val inMsgCell = self.messages.load().inMsg.unwrap() as cell as Cell<MessageRelaxed<Msg>>;
    val inMsg = inMsgCell.load();
    return inMsg.loadBody()
}

/// Loads the in message of the transaction with given type.
///
/// Fails if transaction has no inbound message (`messages.inMsg` is `None`), because this
/// method unwraps it before typed decoding.
///
/// After an inbound message exists, if message type does not match `Msg`, loading fails with
/// exit code 63.
fun Transaction.loadInMsg<Msg>(self) {
    val inMsgCell = self.messages.load().inMsg.unwrap() as cell as Cell<MessageRelaxed<Msg>>;
    return inMsgCell.load()
}

/// Returns consumed gas for ordinary transactions with VM compute phase.
///
/// Hard-fails with [Assert.fail]:
/// - if `description` is not [TransOrd];
/// - if `description.computePh` is [TrComputeSkipped].
fun Transaction.getUsedGas(self) {
    val description = self.description.load();
    if (description is TransOrd) {
        if (description.computePh is TrComputeVm) {
            return description.computePh.data.load().gasUsed as int
        } else {
            Assert.fail("Cannot get used gas for transaction with skipped compute phase")
        }
    } else {
        Assert.fail("Cannot get used gas for non TransOrd")
    }
}

/// Returns action-phase fee based on [Transaction.description] variant.
///
/// Behavior by variant:
/// - [TransOrd] and [TransTickTock] with `action = Some`: returns `action.totalActionFees`;
/// - [TransOrd] and [TransTickTock] with `action = None`: returns `None`;
/// - all other [TransactionDescr] variants: returns `None`.
fun Transaction.getActionFee(self): Maybe<coins> {
    val descr = self.description.load();
    if (descr is TransOrd) {
        val action = descr.action;
        if (action is None) {
            return None{}
        }
        val actionPh = action.value.load();
        return actionPh.totalActionFees
    }
    if (descr is TransTickTock) {
        val action = descr.action;
        if (action is None) {
            return None{}
        }
        val actionPh = action.value.load();
        return actionPh.totalActionFees
    }
    return None{}
}

/// Return address of account of this transaction.
fun Transaction.getAccountAddress(self, workchain: int = BASECHAIN) {
    return address.fromValidBuilder(
        beginCell()
            .storeUint(0b100, 3)
            .storeInt(workchain, 8)
            .storeSlice(self.accountAddr as slice)
    );
}

struct TransactionMessages {
    /// In message.
    inMsg: Maybe<Cell<MessageRelaxedGeneric>>,
    /// Out messages.
    outMsgs: map<uint15, Cell<MessageRelaxedGeneric>>
}

/// Represents merkle update for proof-of-work.
struct (0b0100) MerkleUpdate<X> {
    /// Old hash.
    oldHash: bits256,
    /// New hash.
    newHash: bits256,
    /// Old depth.
    oldDepth: uint16,
    /// New depth.
    newDepth: uint16,
    /// Old value.
    old: X,
    /// New value.
    new: X
}

/// Represents simple hash update.
struct (0b01110010) HashUpdate<X> {
    /// Old hash.
    oldHash: bits256,
    /// New hash.
    newHash: bits256
}

/// Represents merkle proof.
struct (0b0011) MerkleProof<X> {
    /// Virtual hash.
    virtualHash: bits256,
    /// Depth.
    depth: uint16,
    /// Virtual root.
    virtualRoot: X
}

/// Union type for hash updates.
type HashUpdateType<X> = MerkleUpdate<X> | HashUpdate<X> | MerkleProof<X>

/// Represents storage phase of transaction processing.
struct TrStoragePhase {
    /// Storage fees collected.
    storageFeesCollected: coins,
    /// Storage fees due (if any).
    storageFeesDue: Maybe<coins>,
    /// Account status change.
    statusChange: AccStatusChange
}

/// Represents account status changes.
struct (0b0) AccUnchanged // x -> x
struct (0b10) AccFrozen // init -> frozen
struct (0b11) AccDeleted // frozen -> deleted

/// Union type for account status changes.
type AccStatusChange = AccUnchanged | AccFrozen | AccDeleted

/// Represents credit phase of transaction processing.
struct TrCreditPhase {
    /// Due fees collected (if any).
    dueFeesCollected: Maybe<coins>,
    /// Credit amount.
    credit: CurrencyCollection
}

/// Represents compute phase of transaction processing.
struct (0b0) TrComputeSkipped {
    reason: ComputeSkipReason
}
struct (0b1) TrComputeVm {
    success: bool,
    msgStateUsed: bool,
    accountActivated: bool,
    gasFees: coins,
    data: Cell<TrComputeVmRef>
}

struct TrComputeVmRef {
    gasUsed: VarUint7,
    gasLimit: VarUint7,
    gasCredit: Maybe<VarUint3>,
    mode: int8,
    exitCode: int32,
    exitArg: Maybe<int32>,
    vmSteps: uint32,
    vmInitStateHash: bits256,
    vmFinalStateHash: bits256
}

/// Union type for compute phases.
type TrComputePhase = TrComputeSkipped | TrComputeVm

/// Represents reasons for skipping computation.
struct (0b00) CSkipNoState
struct (0b01) CSkipBadState
struct (0b10) CSkipNoGas
struct (0b110) CSkipSuspended

/// Union type for compute skip reasons.
type ComputeSkipReason = CSkipNoState | CSkipBadState | CSkipNoGas | CSkipSuspended

/// Represents action phase of transaction processing.
struct TrActionPhase {
    success: bool,
    valid: bool,
    noFunds: bool,
    statusChange: AccStatusChange,
    totalFwdFees: Maybe<coins>,
    totalActionFees: Maybe<coins>,
    resultCode: int32,
    resultArg: Maybe<int32>,
    totActions: uint16,
    specActions: uint16,
    skippedActions: uint16,
    msgsCreated: uint16,
    actionListHash: bits256,
    totMsgSize: StorageUsed
}

/// Represents bounce phase of transaction processing.
struct (0b00) TrBounceNegfunds
struct (0b01) TrBounceNofunds {
    msgSize: StorageUsed,
    reqFwdFees: coins
}
struct (0b1) TrBounceOk {
    msgSize: StorageUsed,
    msgFees: coins,
    fwdFees: coins
}

/// Union type for bounce phases.
type TrBouncePhase = TrBounceNegfunds | TrBounceNofunds | TrBounceOk

/// Represents ordinary transaction description.
@overflow1023_policy("suppress")
struct (0b0000) TransOrd {
    creditFirst: bool,
    storagePh: Maybe<TrStoragePhase>,
    creditPh: Maybe<TrCreditPhase>,
    computePh: TrComputePhase,
    action: Maybe<Cell<TrActionPhase>>,
    aborted: bool,
    bounce: Maybe<TrBouncePhase>,
    destroyed: bool
}

/// Represents storage transaction description.
struct (0b0001) TransStorage {
    storagePh: TrStoragePhase
}

/// Represents tick-tock transaction description.
@overflow1023_policy("suppress")
struct (0b001) TransTickTock {
    isTock: bool,
    storagePh: TrStoragePhase,
    computePh: TrComputePhase,
    action: Maybe<Cell<TrActionPhase>>,
    aborted: bool,
    destroyed: bool
}

/// Represents split-merge information.
struct SplitMergeInfo {
    curShardPfxLen: uint8, // ## 6
    accSplitDepth: uint8,   // ## 6
    thisAddr: bits256,
    siblingAddr: bits256
}

/// Represents split prepare transaction description.
@overflow1023_policy("suppress")
struct (0b0100) TransSplitPrepare {
    splitInfo: SplitMergeInfo,
    storagePh: Maybe<TrStoragePhase>,
    computePh: TrComputePhase,
    action: Maybe<Cell<TrActionPhase>>,
    aborted: bool,
    destroyed: bool
}

/// Represents split install transaction description.
struct (0b0101) TransSplitInstall {
    splitInfo: SplitMergeInfo,
    prepareTransaction: Cell<Transaction>,
    installed: bool
}

/// Represents merge prepare transaction description.
struct (0b0110) TransMergePrepare {
    splitInfo: SplitMergeInfo,
    storagePh: TrStoragePhase,
    aborted: bool
}

/// Represents merge install transaction description.
@overflow1023_policy("suppress")
struct (0b0111) TransMergeInstall {
    splitInfo: SplitMergeInfo,
    prepareTransaction: Cell<Transaction>,
    storagePh: Maybe<TrStoragePhase>,
    creditPh: Maybe<TrCreditPhase>,
    computePh: TrComputePhase,
    action: Maybe<Cell<TrActionPhase>>,
    aborted: bool,
    destroyed: bool
}

/// Union type for all transaction descriptions.
type TransactionDescr =
    | TransOrd
    | TransStorage
    | TransTickTock
    | TransSplitPrepare
    | TransSplitInstall
    | TransMergePrepare
    | TransMergeInstall

type VarUint7 = int

fun varUintSize(value: int, maxBytes: int): int {
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

fun VarUint7.packToBuilder(self, mutate b: builder) {
    val size = varUintSize(self, 7);
    b.storeUint(size, 3);
    if (size > 0) {
        b.storeUint(self, size * 8);
    }
}

fun VarUint7.unpackFromSlice(mutate s: slice): VarUint7 {
    val size = s.loadUint(3);
    return s.loadUint(size * 8);
}

type VarUint3 = int

fun VarUint3.packToBuilder(self, mutate b: builder) {
    val size = varUintSize(self, 3);
    b.storeUint(size, 2);
    if (size > 0) {
        b.storeUint(self, size * 8);
    }
}

fun VarUint3.unpackFromSlice(mutate s: slice): VarUint3 {
    val size = s.loadUint(2);
    return s.loadUint(size * 8);
}

/// Represents storage used information.
struct StorageUsed {
    cells: VarUint7,
    bits: VarUint7
}

/// Represents storage extra information.
struct (0b000) StorageExtraNone
struct (0b001) StorageExtraDict {
    dictHash: uint256
}

/// Union type for storage extra information.
type StorageExtraInfo = StorageExtraNone | StorageExtraDict

/// Represents storage information.
struct StorageInfo {
    /// Storage used.
    used: StorageUsed,
    /// Storage extra information.
    storageExtra: StorageExtraInfo,
    /// Last paid time.
    lastPaid: uint32,
    /// Due payment (optional).
    duePayment: Maybe<coins>
}

/// Represents account.
struct (0b0) AccountNone
struct (0b1) AccountInfo {
    /// Account address.
    addr: address,
    /// Storage statistics.
    storageStat: StorageInfo,
    /// Account storage.
    storage: AccountStorage
}

/// Union type for account.
type Account = AccountNone | AccountInfo

/// Account state in AccountStorage (block.tlb `AccountState`).
struct (0b00) AccountStateUninit
struct (0b1) AccountStateActive {
    stateInit: StateInit
}
struct (0b01) AccountStateFrozen {
    stateHash: uint256
}

/// Union type for account storage state.
type AccountState = AccountStateUninit | AccountStateActive | AccountStateFrozen

/// Represents account storage.
struct AccountStorage {
    /// Last transaction logical time.
    lastTransLt: uint64,
    /// Account balance.
    balance: CurrencyCollection,
    /// Account state.
    state: AccountState
}

/// Represents a shard account state.
struct ShardAccount {
    account: Cell<Account>,
    lastTransHash: uint256,
    lastTransLt: uint64,
}

/// Represents a list of transactions.
type TransactionList = tuple
