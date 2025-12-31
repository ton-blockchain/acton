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
///     val compute = description.compute_ph;
///     if (compute is TrComputeVm) {
///         println1("VM exit code: {}", compute.exit_code);
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

/// Represents possible account statuses.
type AccountStatus = Uninitialized | Frozen | Active | NonExisting

/// Represents depth and balance information for split accounts.
struct DepthBalanceInfo {
    split_depth: uint8, // #<= 30
    balance: CurrencyCollection
}

/// Represents a transaction in the TON blockchain.
struct (0b0111) Transaction {
    /// Account address as 256-bit hash.
    account_addr: bits256,
    /// Logical time of the transaction.
    lt: uint64,
    /// Hash of the previous transaction.
    prev_trans_hash: bits256,
    /// Logical time of the previous transaction.
    prev_trans_lt: uint64,
    /// Current time (Unix timestamp).
    now: uint32,
    /// Number of outgoing messages.
    outmsg_cnt: uint15,
    /// Account status before transaction.
    orig_status: AccountStatus,
    /// Account status after transaction.
    end_status: AccountStatus,
    /// In and out messages.
    messages: Cell<TransactionMessages>,
    /// Total fees collected.
    total_fees: CurrencyCollection,
    /// State update information.
    state_update: Cell<HashUpdate<Account>>,
    /// Transaction description.
    description: Cell<TransactionDescr>
}

/// Loads the in message body of the transaction with given type.
/// 
/// Note that if message type mismatch, loading will fail with exit code 63.
/// 
/// Example:
/// ```tolk
/// val result = net.send(address, msg, SEND_MODE_PAY_FEES_SEPARATELY);
/// val tx = result.findTransaction<IncreaseCounter>({ to: address }).unwrap();
/// val body = tx.loadBody<IncreaseCounter>();
/// println(body.increaseBy);
/// ```
fun Transaction.loadBody<Msg>(self) {
    val in_msg_cell = self.messages.load().in_msg.unwrap() as cell as Cell<MessageRelaxed<Msg>>;
    val in_msg = in_msg_cell.load();
    return in_msg.loadBody()
}

/// Loads the in message of the transaction with given type.
///
/// Note that if message type mismatch, loading will fail with exit code 63.
fun Transaction.loadInMsg<Msg>(self) {
    val in_msg_cell = self.messages.load().in_msg.unwrap() as cell as Cell<MessageRelaxed<Msg>>;
    return in_msg_cell.load()
}

/// Returns consumed gas for transaction.
fun Transaction.getUsedGas(self) {
    val description = self.description.load();
    if (description is TransOrd) {
        if (description.compute_ph is TrComputeVm) {
            return description.compute_ph.data.load().gas_used as int
        } else {
            Assert.fail("Cannot get used gas for transaction with skipped compute phase")
        }
    } else {
        Assert.fail("Cannot get used gas for non TransOrd")
    }
}

/// Returns fee for action phase of the transaction.
fun Transaction.getActionFee(self): Maybe<coins> {
    val descr = self.description.load();
    if (descr is TransOrd) {
        val action = descr.action;
        if (action is None) {
            return None{}
        }
        val action_ph = action.value.load();
        return action_ph.total_action_fees
    }
    if (descr is TransTickTock) {
        val action = descr.action;
        if (action is None) {
            return None{}
        }
        val action_ph = action.value.load();
        return action_ph.total_action_fees
    }
    return None{}
}

/// Return address of account of this transaction.
fun Transaction.getAccountAddress(self, workchain: int = BASECHAIN) {
    return address.fromValidBuilder(
        beginCell()
            .storeUint(0b100, 3)
            .storeInt(workchain, 8)
            .storeSlice(self.account_addr as slice)
    );
}

struct TransactionMessages {
    /// In message.
    in_msg: Maybe<Cell<MessageRelaxedGeneric>>,
    /// Out messages.
    out_msgs: map<uint15, Cell<MessageRelaxedGeneric>>
}

/// Represents merkle update for proof-of-work.
struct (0b0100) MerkleUpdate<X> {
    /// Old hash.
    old_hash: bits256,
    /// New hash.
    new_hash: bits256,
    /// Old depth.
    old_depth: uint16,
    /// New depth.
    new_depth: uint16,
    /// Old value.
    old: X,
    /// New value.
    new: X
}

/// Represents simple hash update.
struct (0b01110010) HashUpdate<X> {
    /// Old hash.
    old_hash: bits256,
    /// New hash.
    new_hash: bits256
}

/// Represents merkle proof.
struct (0b0011) MerkleProof<X> {
    /// Virtual hash.
    virtual_hash: bits256,
    /// Depth.
    depth: uint16,
    /// Virtual root.
    virtual_root: X
}

/// Union type for hash updates.
type HashUpdateType<X> = MerkleUpdate<X> | HashUpdate<X> | MerkleProof<X>

/// Represents storage phase of transaction processing.
struct TrStoragePhase {
    /// Storage fees collected.
    storage_fees_collected: coins,
    /// Storage fees due (if any).
    storage_fees_due: Maybe<coins>,
    /// Account status change.
    status_change: AccStatusChange
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
    due_fees_collected: Maybe<coins>,
    /// Credit amount.
    credit: CurrencyCollection
}

/// Represents compute phase of transaction processing.
struct (0b0) TrComputeSkipped {
    reason: ComputeSkipReason
}
struct (0b1) TrComputeVm {
    success: bool,
    msg_state_used: bool,
    account_activated: bool,
    gas_fees: coins,
    data: Cell<TrComputeVmRef>
}

struct TrComputeVmRef {
    gas_used: VarUint7,
    gas_limit: VarUint7,
    gas_credit: Maybe<VarUint3>,
    mode: int8,
    exit_code: int32,
    exit_arg: Maybe<int32>,
    vm_steps: uint32,
    vm_init_state_hash: bits256,
    vm_final_state_hash: bits256
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
    no_funds: bool,
    status_change: AccStatusChange,
    total_fwd_fees: Maybe<coins>,
    total_action_fees: Maybe<coins>,
    result_code: int32,
    result_arg: Maybe<int32>,
    tot_actions: uint16,
    spec_actions: uint16,
    skipped_actions: uint16,
    msgs_created: uint16,
    action_list_hash: bits256,
    tot_msg_size: StorageUsed
}

/// Represents bounce phase of transaction processing.
struct (0b00) TrBounceNegfunds
struct (0b01) TrBounceNofunds {
    msg_size: StorageUsed,
    req_fwd_fees: coins
}
struct (0b1) TrBounceOk {
    msg_size: StorageUsed,
    msg_fees: coins,
    fwd_fees: coins
}

/// Union type for bounce phases.
type TrBouncePhase = TrBounceNegfunds | TrBounceNofunds | TrBounceOk

/// Represents ordinary transaction description.
@overflow1023_policy("suppress")
struct (0b0000) TransOrd {
    credit_first: bool,
    storage_ph: Maybe<TrStoragePhase>,
    credit_ph: Maybe<TrCreditPhase>,
    compute_ph: TrComputePhase,
    action: Maybe<Cell<TrActionPhase>>,
    aborted: bool,
    bounce: Maybe<TrBouncePhase>,
    destroyed: bool
}

/// Represents storage transaction description.
struct (0b0001) TransStorage {
    storage_ph: TrStoragePhase
}

/// Represents tick-tock transaction description.
@overflow1023_policy("suppress")
struct (0b001) TransTickTock {
    is_tock: bool,
    storage_ph: TrStoragePhase,
    compute_ph: TrComputePhase,
    action: Maybe<Cell<TrActionPhase>>,
    aborted: bool,
    destroyed: bool
}

/// Represents split-merge information.
struct SplitMergeInfo {
    cur_shard_pfx_len: uint8, // ## 6
    acc_split_depth: uint8,   // ## 6
    this_addr: bits256,
    sibling_addr: bits256
}

/// Represents split prepare transaction description.
@overflow1023_policy("suppress")
struct (0b0100) TransSplitPrepare {
    split_info: SplitMergeInfo,
    storage_ph: Maybe<TrStoragePhase>,
    compute_ph: TrComputePhase,
    action: Maybe<Cell<TrActionPhase>>,
    aborted: bool,
    destroyed: bool
}

/// Represents split install transaction description.
struct (0b0101) TransSplitInstall {
    split_info: SplitMergeInfo,
    prepare_transaction: Cell<Transaction>,
    installed: bool
}

/// Represents merge prepare transaction description.
struct (0b0110) TransMergePrepare {
    split_info: SplitMergeInfo,
    storage_ph: TrStoragePhase,
    aborted: bool
}

/// Represents merge install transaction description.
@overflow1023_policy("suppress")
struct (0b0111) TransMergeInstall {
    split_info: SplitMergeInfo,
    prepare_transaction: Cell<Transaction>,
    storage_ph: Maybe<TrStoragePhase>,
    credit_ph: Maybe<TrCreditPhase>,
    compute_ph: TrComputePhase,
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

fun VarUint7.packToBuilder(self, mutate b: builder) {
    b.storeUint(self, 32) // TODO: correct serialization
}

fun VarUint7.unpackFromSlice(mutate s: slice): VarUint7 {
    val size = s.loadUint(3);
    return s.loadUint(size * 8);
}

type VarUint3 = int

fun VarUint3.packToBuilder(self, mutate b: builder) {
    b.storeUint(self, 16) // TODO: correct serialization
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
    dict_hash: uint256
}

/// Union type for storage extra information.
type StorageExtraInfo = StorageExtraNone | StorageExtraDict

/// Represents storage information.
struct StorageInfo {
    /// Storage used.
    used: StorageUsed,
    /// Storage extra information.
    storage_extra: StorageExtraInfo,
    /// Last paid time.
    last_paid: uint32,
    /// Due payment (optional).
    due_payment: Maybe<coins>
}

/// Represents account.
struct (0b0) AccountNone
struct (0b1) AccountInfo {
    /// Account address.
    addr: address,
    /// Storage statistics.
    storage_stat: StorageInfo,
    /// Account storage.
    storage: AccountStorage
}

/// Union type for account.
type Account = AccountNone | AccountInfo

/// Represents account storage.
struct AccountStorage {
    /// Last transaction logical time.
    last_trans_lt: uint64,
    /// Account balance.
    balance: CurrencyCollection,
    /// Account state.
    state: AccountStatus
}

/// Represents a list of transactions.
type TransactionList = tuple
