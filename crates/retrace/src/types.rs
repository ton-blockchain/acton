use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tycho_types::cell::Cell;
use tycho_types::models::{IntAddr, OutAction, StdAddr};

/// Minimal "handle" for locating a transaction on the TON blockchain.
///
/// A tuple of (lt, hash, address) is guaranteed to be unique and can be
/// passed to RPC methods such as `get_account_transactions` to retrieve
/// the full on‑chain record.
///
/// Can be obtained by [`crate::find_base_tx_by_hash`].
#[derive(Debug, Clone)]
pub struct BaseTxInfo {
    /// Logical‑time of the transaction.
    pub lt: u64,
    /// Raw 256‑bit hash of the transaction `BoC`.
    pub hash: [u8; 32],
    /// Contract address that issued / owns the transaction.
    pub address: StdAddr,
}

// --- TonCenter v3 API Types ---

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct TransactionData {
    pub transactions: Vec<Transaction>,
    pub address_book: HashMap<String, AddressBookEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct AddressBookEntry {
    pub user_friendly: String,
    pub domain: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct Transaction {
    pub account: String,
    pub hash: String,
    pub lt: String,
    pub now: u64,
    pub mc_block_seqno: u64,
    pub trace_id: String,
    pub prev_trans_hash: String,
    pub prev_trans_lt: String,
    pub orig_status: String,
    pub end_status: String,
    pub total_fees: String,
    pub total_fees_extra_currencies: HashMap<String, serde_json::Value>,
    pub description: Description,
    pub block_ref: BlockRef,
    pub account_state_before: AccountState,
    pub account_state_after: AccountState,
    pub emulated: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct Description {
    #[serde(rename = "type")]
    pub desc_type: String,
    pub aborted: bool,
    pub destroyed: bool,
    pub credit_first: bool,
    pub storage_ph: Option<StoragePhase>,
    pub credit_ph: Option<CreditPhase>,
    pub compute_ph: Option<ComputePhase>,
    pub action: Option<ActionPhase>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct StoragePhase {
    pub storage_fees_collected: String,
    pub status_change: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct CreditPhase {
    pub credit: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ComputePhase {
    pub skipped: bool,
    pub success: bool,
    pub msg_state_used: bool,
    pub account_activated: bool,
    pub gas_fees: String,
    pub gas_used: String,
    pub gas_limit: String,
    pub mode: i32,
    pub exit_code: i32,
    pub vm_steps: u32,
    pub vm_init_state_hash: String,
    pub vm_final_state_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ActionPhase {
    pub success: bool,
    pub valid: bool,
    pub no_funds: bool,
    pub status_change: String,
    pub result_code: i32,
    pub tot_actions: i32,
    pub spec_actions: i32,
    pub skipped_actions: i32,
    pub msgs_created: i32,
    pub action_list_hash: String,
    pub tot_msg_size: TotalMsgSize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct TotalMsgSize {
    pub cells: String,
    pub bits: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct BlockRef {
    pub workchain: i32,
    pub shard: String,
    pub seqno: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct AccountState {
    pub hash: String,
    pub balance: Option<String>,
    pub extra_currencies: Option<HashMap<String, serde_json::Value>>,
    pub account_status: Option<String>,
    pub frozen_hash: Option<String>,
    pub data_hash: Option<String>,
    pub code_hash: Option<String>,
}

// --- Retrace Internal Types ---

/// Detailed information about the compute phase of a transaction.
///
/// This enum captures whether the compute phase was skipped or executed,
/// along with detailed statistics like gas usage and exit codes.
///
/// # Example
///
/// ```ignore
/// match result.emulated_tx.compute_info {
///     ComputeInfo::Skipped => println!("Compute phase skipped"),
///     ComputeInfo::Success { exit_code, gas_used, .. } => {
///         println!("Exit code: {}, Gas used: {}", exit_code, gas_used);
///     }
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ComputeInfo {
    /// Compute phase was skipped (e.g., account is uninitialized or not enough funds for storage).
    #[serde(rename = "skipped")]
    Skipped,
    /// Compute phase was executed.
    #[serde(rename = "success")]
    Success {
        /// Whether the compute phase was successful.
        success: bool,
        /// VM exit code.
        #[serde(rename = "exitCode")]
        exit_code: i32,
        /// Number of VM steps executed.
        #[serde(rename = "vmSteps")]
        vm_steps: u32,
        /// Amount of gas used.
        #[serde(rename = "gasUsed")]
        gas_used: u64,
        /// Fees paid for gas.
        #[serde(rename = "gasFees")]
        gas_fees: u64,
    },
}

/// Information about the incoming message that triggered the transaction.
///
/// # Example
///
/// ```ignore
/// let in_msg = &result.in_msg;
/// println!("From: {:?}, To: {:?}, Amount: {:?}", in_msg.sender, in_msg.contract, in_msg.amount);
/// ```
#[derive(Debug, Clone)]
pub struct TraceInMessage {
    /// Sender address (None for external messages).
    pub sender: Option<IntAddr>,
    /// Contract address that received the message.
    pub contract: IntAddr,
    /// Amount of nanoton sent with the message.
    pub amount: Option<u64>,
    /// Opcode extracted from the message body.
    pub opcode: Option<u32>,
}

/// Detailed information about the emulated transaction.
///
/// Contains the raw transaction object, execution timing, and full logs.
#[derive(Debug, Clone)]
pub struct TraceEmulatedTx {
    /// Emulated transaction.
    pub raw: tycho_types::models::Transaction,
    /// Unix time of the transaction execution.
    pub utime: u64,
    /// Logical‑time of the transaction.
    pub lt: u64,
    /// Information about the compute phase (gas usage, exit code, etc).
    pub compute_info: ComputeInfo,
    /// Detailed logs from the sandbox executor.
    pub executor_logs: Arc<str>,
    /// List of outgoing actions generated by the transaction.
    pub actions: Vec<OutAction>,
    /// Final state of the `c5` control register (action list cell).
    pub c5: Option<Cell>,
    /// Detailed VM execution logs.
    pub vm_logs: Arc<str>,
}

/// Breakdown of money movements and fees within the transaction.
///
/// All values are in nanoton (10^-9 TON).
///
/// # Example
///
/// ```ignore
/// let money = &result.money;
/// println!("Fees: {} nanoton", money.total_fees);
/// println!("Balance after: {} nanoton", money.balance_after);
/// ```
#[derive(Debug, Clone)]
pub struct TraceMoneyResult {
    /// Balance of the account **before** the transaction execution.
    pub balance_before: u64,
    /// Sum of all nanotons sent via *internal* outgoing messages.
    /// External messages are excluded as they carry no value.
    pub sent_total: u64,
    /// Total fees of the transaction (including storage, gas and action fees).
    pub total_fees: u64,
    /// Balance of the account **after** the transaction execution and fee deduction.
    pub balance_after: u64,
}

/// The final report containing all data from the transaction retrace process.
///
/// This is the primary output of the [`crate::retrace`] function.
///
/// # Example
///
/// ```ignore
/// let result = retrace(Network::Mainnet, hash, libs).await?;
/// if result.state_update_hash_ok {
///     println!("Deterministic replay verified!");
/// }
/// ```
#[derive(Debug, Clone)]
pub struct TraceResult {
    /// True if the emulated state‑update hash matches the one recorded on‑chain.
    pub state_update_hash_ok: bool,
    /// The actual code cell used for execution (resolved library code if exotic).
    pub code_cell: Option<Cell>,
    /// The code cell as stored in the account state (may be an exotic library cell).
    pub original_code_cell: Option<Cell>,
    /// Information about the message that triggered this transaction.
    pub in_msg: TraceInMessage,
    /// Detailed breakdown of balances and fees.
    pub money: TraceMoneyResult,
    /// Full details of the emulated transaction execution results.
    pub emulated_tx: TraceEmulatedTx,
}

// --- API State Types ---

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub(crate) enum StateFromAPI {
    #[serde(rename = "uninit")]
    Uninit,
    #[serde(rename = "active")]
    Active {
        data: Option<String>,
        code: Option<String>,
    },
    #[serde(rename = "frozen")]
    Frozen {
        #[serde(rename = "stateHash")]
        state_hash: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct AccountFromAPI {
    pub balance: AccountBalance,
    pub state: StateFromAPI,
    pub last: Option<LastTxRef>,
    #[serde(rename = "storageStat")]
    pub storage_stat: Option<StorageStat>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct AccountBalance {
    pub coins: String,
    pub currencies: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct LastTxRef {
    pub lt: String,
    pub hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct StorageStat {
    #[serde(rename = "lastPaid")]
    pub last_paid: u64,
    #[serde(rename = "duePayment")]
    pub due_payment: Option<String>,
    pub used: StorageUsed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct StorageUsed {
    pub bits: u64,
    pub cells: u64,
    #[serde(rename = "publicCells")]
    pub public_cells: Option<u64>,
}

// --- Blocks API ---

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct BlocksResponse {
    pub blocks: Vec<Block>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct Block {
    pub after_merge: bool,
    pub after_split: bool,
    pub before_split: bool,
    pub created_by: String,
    pub end_lt: String,
    pub file_hash: String,
    pub flags: u32,
    pub gen_catchain_seqno: u32,
    pub gen_utime: String,
    pub global_id: i32,
    pub key_block: bool,
    pub master_ref_seqno: Option<u32>,
    pub masterchain_block_ref: BlockRef,
    pub min_ref_mc_seqno: Option<u32>,
    pub prev_blocks: Vec<BlockRef>,
    pub prev_key_block_seqno: u32,
    pub rand_seed: String,
    pub root_hash: String,
    pub seqno: u32,
    pub shard: String,
    pub start_lt: String,
    pub tx_count: u32,
    pub validator_list_hash_short: i64,
    pub version: u32,
    pub vert_seqno: u32,
    pub vert_seqno_incr: bool,
    pub want_merge: bool,
    pub want_split: bool,
    pub workchain: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ShardInfo {
    pub workchain: i32,
    pub shard: String,
    pub seqno: u32,
    pub transactions: Vec<BaseTxInfoShort>,
    #[serde(rename = "fileHash")]
    pub file_hash: String,
    #[serde(rename = "rootHash")]
    pub root_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct BaseTxInfoShort {
    pub lt: String,
    pub hash: String,
    pub account: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct BlockInfo {
    pub shards: Vec<ShardInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct RawTransaction {
    pub block: RawTransactionBlock,
    pub tx: tycho_types::models::Transaction,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct RawTransactionBlock {
    pub workchain: i32,
    pub seqno: u32,
    pub shard: String,
    #[serde(rename = "rootHash")]
    pub root_hash: String,
    #[serde(rename = "fileHash")]
    pub file_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct TransactionTransactionsResponse {
    pub blocks: Vec<RawTransactionBlock>,
    pub boc: String,
}
