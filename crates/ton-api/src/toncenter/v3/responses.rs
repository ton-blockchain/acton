use super::StringOrNumber;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

pub type AddressBook = HashMap<String, AddressBookRow>;
pub type Metadata = HashMap<String, AddressMetadata>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddressBookRow {
    #[serde(default)]
    pub domain: Option<String>,
    #[serde(default)]
    pub interfaces: Option<Vec<String>>,
    #[serde(default)]
    pub user_friendly: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddressMetadata {
    #[serde(default)]
    pub is_indexed: Option<bool>,
    #[serde(default)]
    pub token_info: Vec<TokenInfo>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TokenInfo {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub valid: Option<bool>,
    #[serde(default, rename = "type", skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub symbol: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nft_index: Option<String>,
    #[serde(default)]
    pub extra: HashMap<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountStatesResponse {
    pub accounts: Vec<AccountStateFull>,
    #[serde(default)]
    pub address_book: AddressBook,
    #[serde(default)]
    pub metadata: Metadata,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TracesResponse {
    pub traces: Vec<Trace>,
    #[serde(default)]
    pub address_book: AddressBook,
    #[serde(default)]
    pub metadata: Metadata,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionsResponse {
    pub transactions: Vec<Transaction>,
    #[serde(default)]
    pub address_book: AddressBook,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockId {
    pub workchain: i32,
    pub shard: String,
    pub seqno: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Block {
    pub workchain: i32,
    pub shard: String,
    pub seqno: u32,
    pub root_hash: String,
    pub file_hash: String,
    pub start_lt: String,
    pub end_lt: String,
    pub gen_utime: StringOrNumber,
    #[serde(default)]
    pub masterchain_block_ref: Option<BlockId>,
    #[serde(default)]
    pub prev_blocks: Vec<BlockId>,
    #[serde(default)]
    pub after_merge: Option<bool>,
    #[serde(default)]
    pub after_split: Option<bool>,
    #[serde(default)]
    pub before_split: Option<bool>,
    #[serde(default)]
    pub created_by: Option<String>,
    #[serde(default)]
    pub flags: Option<i32>,
    #[serde(default)]
    pub gen_catchain_seqno: Option<i32>,
    #[serde(default)]
    pub global_id: Option<i32>,
    #[serde(default)]
    pub key_block: Option<bool>,
    #[serde(default)]
    pub master_ref_seqno: Option<i32>,
    #[serde(default)]
    pub min_ref_mc_seqno: Option<i32>,
    #[serde(default)]
    pub prev_key_block_seqno: Option<i32>,
    #[serde(default)]
    pub rand_seed: Option<String>,
    #[serde(default)]
    pub tx_count: Option<i32>,
    #[serde(default)]
    pub validator_list_hash_short: Option<i32>,
    #[serde(default)]
    pub version: Option<i32>,
    #[serde(default)]
    pub vert_seqno: Option<i32>,
    #[serde(default)]
    pub vert_seqno_incr: Option<bool>,
    #[serde(default)]
    pub want_merge: Option<bool>,
    #[serde(default)]
    pub want_split: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlocksResponse {
    pub blocks: Vec<Block>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JettonMaster {
    pub address: String,
    #[serde(default)]
    pub admin_address: Option<String>,
    #[serde(default)]
    pub code_hash: Option<String>,
    #[serde(default)]
    pub data_hash: Option<String>,
    #[serde(default)]
    pub jetton_content: HashMap<String, Value>,
    #[serde(default)]
    pub jetton_wallet_code_hash: Option<String>,
    #[serde(default)]
    pub last_transaction_lt: Option<String>,
    #[serde(default)]
    pub mintable: Option<bool>,
    #[serde(default)]
    pub total_supply: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JettonMastersResponse {
    pub jetton_masters: Vec<JettonMaster>,
    #[serde(default)]
    pub address_book: AddressBook,
    #[serde(default)]
    pub metadata: Metadata,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JettonWalletsResponse {
    pub jetton_wallets: Vec<JettonWallet>,
    #[serde(default)]
    pub address_book: AddressBook,
    #[serde(default)]
    pub metadata: Metadata,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NftItem {
    pub address: String,
    #[serde(default)]
    pub auction_contract_address: Option<String>,
    #[serde(default)]
    pub code_hash: Option<String>,
    #[serde(default)]
    pub collection: Option<NftCollectionRef>,
    #[serde(default)]
    pub collection_address: Option<String>,
    #[serde(default)]
    pub content: HashMap<String, Value>,
    #[serde(default)]
    pub data_hash: Option<String>,
    #[serde(default)]
    pub index: Option<String>,
    #[serde(default)]
    pub init: Option<bool>,
    #[serde(default)]
    pub last_transaction_lt: Option<String>,
    #[serde(default)]
    pub on_sale: Option<bool>,
    #[serde(default)]
    pub owner_address: Option<String>,
    #[serde(default)]
    pub real_owner: Option<String>,
    #[serde(default)]
    pub sale_contract_address: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NftCollectionRef {
    pub address: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NftItemsResponse {
    pub nft_items: Vec<NftItem>,
    #[serde(default)]
    pub address_book: AddressBook,
    #[serde(default)]
    pub metadata: Metadata,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SendMessageResult {
    pub message_hash: String,
    pub message_hash_norm: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunGetMethodResult {
    pub gas_used: StringOrNumber,
    pub exit_code: i32,
    pub stack: Vec<StackEntity>,
    #[serde(default)]
    pub vm_log: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StackEntity {
    #[serde(rename = "type")]
    pub kind: String,
    pub value: StackValue,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum StackValue {
    Entries(Vec<StackEntity>),
    Json(Value),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct V2AddressInformation {
    pub balance: String,
    #[serde(default)]
    pub code: Option<String>,
    #[serde(default)]
    pub data: Option<String>,
    #[serde(default)]
    pub frozen_hash: Option<String>,
    #[serde(default)]
    pub last_transaction_hash: Option<String>,
    #[serde(default)]
    pub last_transaction_lt: Option<String>,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequestError {
    /// Present for API errors, but omitted by request-body validation errors (HTTP 422).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub code: Option<i32>,
    pub error: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraceNode {
    #[serde(default)]
    pub children: Vec<TraceNode>,
    #[serde(default)]
    pub in_msg: Option<Message>,
    #[serde(default)]
    pub in_msg_hash: Option<String>,
    #[serde(default)]
    pub transaction: Option<Transaction>,
    #[serde(default)]
    pub tx_hash: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Action {
    #[serde(default)]
    pub accounts: Vec<String>,
    #[serde(default)]
    pub action_id: Option<String>,
    #[serde(default)]
    pub details: Option<Value>,
    #[serde(default)]
    pub end_lt: Option<String>,
    #[serde(default)]
    pub end_utime: Option<u32>,
    #[serde(default)]
    pub finality: Option<String>,
    #[serde(default)]
    pub start_lt: Option<String>,
    #[serde(default)]
    pub start_utime: Option<u32>,
    #[serde(default)]
    pub success: Option<bool>,
    #[serde(default)]
    pub trace_end_lt: Option<String>,
    #[serde(default)]
    pub trace_end_utime: Option<u32>,
    #[serde(default)]
    pub trace_external_hash: Option<String>,
    #[serde(default)]
    pub trace_external_hash_norm: Option<String>,
    #[serde(default)]
    pub trace_id: Option<String>,
    #[serde(default)]
    pub trace_mc_seqno_end: Option<u32>,
    #[serde(default)]
    pub transactions: Vec<String>,
    #[serde(default)]
    pub transactions_full: Vec<Transaction>,
    #[serde(default, rename = "type")]
    pub kind: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JettonWallet {
    pub address: String,
    pub balance: String,
    #[serde(default)]
    pub code_hash: Option<String>,
    #[serde(default)]
    pub data_hash: Option<String>,
    pub jetton: String,
    pub last_transaction_lt: String,
    #[serde(default)]
    pub mintless_info: Option<Value>,
    pub owner: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountStateFull {
    pub address: String,
    #[serde(default)]
    pub account_state_hash: Option<String>,
    #[serde(default)]
    pub balance: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub code_boc: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub code_hash: Option<String>,
    #[serde(default)]
    pub contract_methods: Vec<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data_boc: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data_hash: Option<String>,
    #[serde(default)]
    pub extra_currencies: Option<HashMap<String, String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub frozen_hash: Option<String>,
    #[serde(default)]
    pub interfaces: Option<Vec<String>>,
    #[serde(default)]
    pub last_transaction_hash: Option<String>,
    #[serde(default)]
    pub last_transaction_lt: Option<String>,
    pub status: String,
}

/// Transaction returned by `/transactions`, `/transactionsByMessage`, and traces.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Transaction {
    pub account: String,
    pub hash: String,
    pub lt: String,
    #[serde(default)]
    pub block_ref: Option<BlockId>,
    #[serde(default)]
    pub now: u32,
    #[serde(default)]
    pub mc_block_seqno: Option<u32>,
    #[serde(default)]
    pub emulated: Option<bool>,
    #[serde(default)]
    pub finality: Option<String>,
    #[serde(default)]
    pub prev_trans_hash: Option<String>,
    #[serde(default)]
    pub prev_trans_lt: Option<String>,
    #[serde(default)]
    pub orig_status: Option<String>,
    #[serde(default)]
    pub end_status: Option<String>,
    #[serde(default)]
    pub total_fees: Option<String>,
    #[serde(default)]
    pub total_fees_extra_currencies: HashMap<String, String>,
    #[serde(default)]
    pub trace_external_hash: Option<String>,
    #[serde(default)]
    pub trace_id: Option<String>,
    #[serde(default)]
    pub child_transactions: Vec<String>,
    #[serde(default)]
    pub description: Option<TransactionDescr>,
    #[serde(default)]
    pub in_msg: Option<Message>,
    #[serde(default)]
    pub out_msgs: Vec<Message>,
    #[serde(default)]
    pub account_state_before: Option<AccountState>,
    #[serde(default)]
    pub account_state_after: Option<AccountState>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountState {
    #[serde(default)]
    pub hash: Option<String>,
    #[serde(default)]
    pub account_status: Option<String>,
    #[serde(default)]
    pub balance: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub code_boc: Option<String>,
    #[serde(default)]
    pub code_hash: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data_boc: Option<String>,
    #[serde(default)]
    pub data_hash: Option<String>,
    #[serde(default)]
    pub extra_currencies: Option<HashMap<String, String>>,
    #[serde(default)]
    pub frozen_hash: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionDescr {
    #[serde(default, rename = "type")]
    pub kind: Option<String>,
    #[serde(default)]
    pub aborted: Option<bool>,
    #[serde(default)]
    pub destroyed: Option<bool>,
    #[serde(default)]
    pub credit_first: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub compute_ph: Option<ComputePhase>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub action: Option<ActionPhase>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub storage_ph: Option<StoragePhase>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub credit_ph: Option<CreditPhase>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bounce: Option<Value>,
    #[serde(default)]
    pub installed: Option<bool>,
    #[serde(default)]
    pub is_tock: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub split_info: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreditPhase {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub due_fees_collected: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub credit: Option<String>,
    #[serde(default)]
    pub credit_extra_currencies: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComputePhase {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub skipped: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub success: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub msg_state_used: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub account_activated: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gas_fees: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gas_used: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gas_limit: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gas_credit: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mode: Option<i8>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exit_code: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exit_arg: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vm_steps: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vm_init_state_hash: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vm_final_state_hash: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionPhase {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub success: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub valid: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub no_funds: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status_change: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result_code: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result_arg: Option<i32>,
    #[serde(
        default,
        alias = "total_actions",
        skip_serializing_if = "Option::is_none"
    )]
    pub tot_actions: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spec_actions: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub skipped_actions: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub msgs_created: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub total_fwd_fees: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub total_action_fees: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub action_list_hash: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tot_msg_size: Option<MsgSize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MsgSize {
    #[serde(default)]
    pub cells: Option<String>,
    #[serde(default)]
    pub bits: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoragePhase {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub storage_fees_collected: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub storage_fees_due: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status_change: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    #[serde(default)]
    pub hash: Option<String>,
    #[serde(default)]
    pub hash_norm: Option<String>,
    #[serde(default)]
    pub source: Option<String>,
    #[serde(default)]
    pub destination: Option<String>,
    #[serde(default)]
    pub value: Option<String>,
    #[serde(default)]
    pub value_extra_currencies: Option<HashMap<String, String>>,
    #[serde(default)]
    pub fwd_fee: Option<String>,
    #[serde(default)]
    pub ihr_fee: Option<String>,
    #[serde(default)]
    pub created_lt: Option<String>,
    #[serde(default)]
    pub created_at: Option<String>,
    #[serde(default)]
    pub decoded_opcode: Option<String>,
    #[serde(default)]
    pub extra_flags: Option<String>,
    #[serde(default)]
    pub ihr_disabled: Option<bool>,
    #[serde(default)]
    pub bounce: Option<bool>,
    #[serde(default)]
    pub bounced: Option<bool>,
    #[serde(default)]
    pub import_fee: Option<String>,
    #[serde(default)]
    pub in_msg_tx_hash: Option<String>,
    #[serde(default)]
    pub opcode: Option<StringOrNumber>,
    #[serde(default)]
    pub out_msg_tx_hash: Option<String>,
    #[serde(default)]
    pub message_content: Option<MessageContent>,
    #[serde(default)]
    pub init_state: Option<MessageContent>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageContent {
    #[serde(default)]
    pub hash: Option<String>,
    #[serde(default)]
    pub body: Option<String>,
    #[serde(default)]
    pub decoded: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Trace {
    pub trace_id: String,
    pub transactions_order: Vec<String>,
    pub transactions: HashMap<String, Transaction>,
    #[serde(default)]
    pub is_incomplete: bool,
    #[serde(default)]
    pub actions: Vec<Action>,
    #[serde(default)]
    pub end_lt: Option<String>,
    #[serde(default)]
    pub end_utime: Option<u32>,
    #[serde(default)]
    pub external_hash: Option<String>,
    #[serde(default)]
    pub mc_seqno_end: Option<String>,
    #[serde(default)]
    pub mc_seqno_start: Option<String>,
    #[serde(default)]
    pub start_lt: Option<String>,
    #[serde(default)]
    pub start_utime: Option<u32>,
    #[serde(default)]
    pub trace: Option<TraceNode>,
    #[serde(default)]
    pub trace_info: Option<TraceInfo>,
    #[serde(default)]
    pub warning: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraceInfo {
    pub transactions: usize,
    pub messages: usize,
    pub pending_messages: usize,
    pub trace_state: String,
    pub classification_state: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trace_accepts_additional_openapi_fields() {
        let trace: Trace = serde_json::from_value(serde_json::json!({
            "trace_id": "trace",
            "transactions_order": [],
            "transactions": {},
            "is_incomplete": false,
            "warning": "upstream field not consumed by this response projection"
        }))
        .expect("v3 trace response must accept the full upstream envelope");

        assert_eq!(trace.trace_id, "trace");
    }
}
