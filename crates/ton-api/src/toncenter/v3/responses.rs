use super::StringOrNumber;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

fn deserialize_null_default<'de, D, T>(deserializer: D) -> Result<T, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de> + Default,
{
    Ok(Option::<T>::deserialize(deserializer)?.unwrap_or_default())
}

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
    pub is_indexed: bool,
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub is_nsfw: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub is_scam: Option<bool>,
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
    #[serde(default, deserialize_with = "deserialize_null_default")]
    pub traces: Vec<Trace>,
    #[serde(default)]
    pub address_book: AddressBook,
    #[serde(default)]
    pub metadata: Metadata,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionsResponse {
    pub actions: Vec<Action>,
    #[serde(default)]
    pub address_book: AddressBook,
    #[serde(default)]
    pub metadata: Metadata,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DnsRecord {
    pub nft_item_address: String,
    pub nft_item_owner: Option<String>,
    pub domain: String,
    pub dns_next_resolver: Option<String>,
    pub dns_wallet: Option<String>,
    pub dns_site_adnl: Option<String>,
    pub dns_storage_bag_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DnsRecordsResponse {
    pub records: Vec<DnsRecord>,
    #[serde(default)]
    pub address_book: AddressBook,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JettonTransfer {
    pub query_id: String,
    pub source: String,
    pub destination: String,
    pub amount: String,
    pub source_wallet: String,
    pub jetton_master: String,
    pub transaction_hash: String,
    pub transaction_lt: String,
    pub transaction_now: i64,
    pub transaction_aborted: bool,
    pub response_destination: Option<String>,
    pub custom_payload: Option<String>,
    pub decoded_custom_payload: Option<Value>,
    pub forward_ton_amount: Option<String>,
    pub forward_payload: Option<String>,
    pub decoded_forward_payload: Option<Value>,
    pub trace_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JettonTransfersResponse {
    pub jetton_transfers: Vec<JettonTransfer>,
    #[serde(default)]
    pub address_book: AddressBook,
    #[serde(default)]
    pub metadata: Metadata,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JettonBurn {
    pub query_id: String,
    pub owner: String,
    pub jetton_wallet: String,
    pub jetton_master: String,
    pub transaction_hash: String,
    pub transaction_lt: String,
    pub transaction_now: i64,
    pub transaction_aborted: bool,
    pub amount: String,
    pub response_destination: Option<String>,
    pub custom_payload: Option<String>,
    pub decoded_custom_payload: Option<Value>,
    pub trace_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JettonBurnsResponse {
    pub jetton_burns: Vec<JettonBurn>,
    #[serde(default)]
    pub address_book: AddressBook,
    #[serde(default)]
    pub metadata: Metadata,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NftCollection {
    pub address: String,
    pub owner_address: Option<String>,
    pub last_transaction_lt: String,
    pub next_item_index: String,
    pub collection_content: HashMap<String, Value>,
    pub data_hash: String,
    pub code_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NftCollectionsResponse {
    pub nft_collections: Vec<NftCollection>,
    #[serde(default)]
    pub address_book: AddressBook,
    #[serde(default)]
    pub metadata: Metadata,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NftTransfer {
    pub query_id: String,
    pub nft_address: String,
    pub nft_collection: String,
    pub transaction_hash: String,
    pub transaction_lt: String,
    pub transaction_now: i64,
    pub transaction_aborted: bool,
    pub old_owner: String,
    pub new_owner: String,
    pub response_destination: Option<String>,
    pub custom_payload: Option<String>,
    pub decoded_custom_payload: Option<Value>,
    pub forward_amount: Option<String>,
    pub forward_payload: Option<String>,
    pub decoded_forward_payload: Option<Value>,
    pub trace_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NftTransfersResponse {
    pub nft_transfers: Vec<NftTransfer>,
    #[serde(default)]
    pub address_book: AddressBook,
    #[serde(default)]
    pub metadata: Metadata,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NftSale {
    #[serde(rename = "type")]
    pub kind: String,
    pub address: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nft_address: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nft_owner_address: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub marketplace_address: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created_at: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_transaction_lt: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub code_hash: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data_hash: Option<String>,
    pub details: Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nft_item: Option<NftItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NftSalesResponse {
    pub nft_sales: Vec<NftSale>,
    #[serde(default)]
    pub address_book: AddressBook,
    #[serde(default)]
    pub metadata: Metadata,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MultisigOrderAction {
    pub destination: Option<String>,
    pub value: Option<String>,
    pub body_raw: Value,
    pub parsed: bool,
    pub error: Option<String>,
    pub parsed_body: Option<Value>,
    pub parsed_body_type: String,
    pub send_mode: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MultisigOrder {
    pub address: String,
    pub multisig_address: String,
    pub order_seqno: Option<String>,
    pub threshold: Option<i32>,
    pub sent_for_execution: Option<bool>,
    pub approvals_mask: Option<String>,
    pub approvals_num: Option<i32>,
    pub expiration_date: Option<u64>,
    pub order_boc: Option<String>,
    #[serde(default)]
    pub signers: Vec<String>,
    pub last_transaction_lt: String,
    pub code_hash: Option<String>,
    pub data_hash: Option<String>,
    #[serde(default, deserialize_with = "deserialize_null_default")]
    pub actions: Vec<MultisigOrderAction>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Multisig {
    pub address: String,
    pub next_order_seqno: Option<String>,
    pub threshold: Option<i32>,
    #[serde(default)]
    pub signers: Vec<String>,
    #[serde(default)]
    pub proposers: Vec<String>,
    pub last_transaction_lt: String,
    pub code_hash: Option<String>,
    pub data_hash: Option<String>,
    #[serde(default)]
    pub orders: Vec<MultisigOrder>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MultisigOrdersResponse {
    pub orders: Vec<MultisigOrder>,
    #[serde(default)]
    pub address_book: AddressBook,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MultisigsResponse {
    pub multisigs: Vec<Multisig>,
    #[serde(default)]
    pub address_book: AddressBook,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VestingInfo {
    pub address: Option<String>,
    pub start_time: Option<i64>,
    pub total_duration: Option<i64>,
    pub unlock_period: Option<i64>,
    pub cliff_duration: Option<i64>,
    pub sender_address: Option<String>,
    pub owner_address: Option<String>,
    pub total_amount: Option<String>,
    #[serde(default)]
    pub whitelist: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VestingContractsResponse {
    pub vesting_contracts: Vec<VestingInfo>,
    #[serde(default)]
    pub address_book: AddressBook,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionsResponse {
    pub transactions: Vec<Transaction>,
    #[serde(default)]
    pub address_book: AddressBook,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessagesResponse {
    pub messages: Vec<Message>,
    #[serde(default)]
    pub address_book: AddressBook,
    #[serde(default)]
    pub metadata: Metadata,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WalletStatesResponse {
    pub wallets: Vec<WalletState>,
    #[serde(default)]
    pub address_book: AddressBook,
    #[serde(default)]
    pub metadata: Metadata,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WalletState {
    pub address: String,
    pub is_wallet: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wallet_type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seqno: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wallet_id: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub balance: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub extra_currencies: Option<HashMap<String, String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub is_signature_allowed: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub code_hash: Option<String>,
    #[serde(default)]
    pub last_transaction_hash: Option<String>,
    #[serde(default)]
    pub last_transaction_lt: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountBalance {
    pub account: String,
    pub balance: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EstimateFeeResult {
    pub source_fees: EstimatedFee,
    pub destination_fees: Vec<EstimatedFee>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct EstimatedFee {
    pub in_fwd_fee: u64,
    pub storage_fee: u64,
    pub gas_fee: u64,
    pub fwd_fee: u64,
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
    pub masterchain_block_ref: BlockId,
    #[serde(default)]
    pub prev_blocks: Vec<BlockId>,
    pub after_merge: bool,
    pub after_split: bool,
    pub before_split: bool,
    pub created_by: String,
    pub flags: i32,
    pub gen_catchain_seqno: i32,
    pub global_id: i32,
    pub key_block: bool,
    pub master_ref_seqno: i32,
    pub min_ref_mc_seqno: i32,
    pub prev_key_block_seqno: i32,
    pub rand_seed: String,
    pub tx_count: i64,
    pub validator_list_hash_short: i32,
    pub version: i64,
    pub vert_seqno: i32,
    pub vert_seqno_incr: bool,
    pub want_merge: bool,
    pub want_split: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlocksResponse {
    pub blocks: Vec<Block>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MasterchainInfo {
    pub first: Block,
    pub last: Block,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JettonMaster {
    pub address: String,
    #[serde(default)]
    pub admin_address: Option<String>,
    pub code_hash: String,
    pub data_hash: String,
    #[serde(default)]
    pub jetton_content: HashMap<String, Value>,
    pub jetton_wallet_code_hash: String,
    pub last_transaction_lt: String,
    pub mintable: bool,
    pub total_supply: String,
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
    pub code_hash: String,
    #[serde(default)]
    pub collection: Option<NftCollectionRef>,
    #[serde(default)]
    pub collection_address: Option<String>,
    #[serde(default)]
    pub content: HashMap<String, Value>,
    pub data_hash: String,
    pub index: String,
    pub init: bool,
    pub last_transaction_lt: String,
    pub on_sale: bool,
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
pub struct V2WalletInformation {
    pub balance: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wallet_type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seqno: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wallet_id: Option<i64>,
    pub last_transaction_lt: String,
    pub last_transaction_hash: String,
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
    pub action_id: String,
    pub details: Value,
    pub end_lt: String,
    pub end_utime: u32,
    pub finality: String,
    pub start_lt: String,
    pub start_utime: u32,
    #[serde(default)]
    pub success: Option<bool>,
    pub trace_end_lt: String,
    pub trace_end_utime: u32,
    #[serde(default)]
    pub trace_external_hash: Option<String>,
    #[serde(default)]
    pub trace_external_hash_norm: Option<String>,
    #[serde(default)]
    pub trace_id: Option<String>,
    pub trace_mc_seqno_end: u32,
    #[serde(default)]
    pub transactions: Vec<String>,
    #[serde(default)]
    pub transactions_full: Vec<Transaction>,
    #[serde(rename = "type")]
    pub kind: String,
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
    pub account_state_hash: String,
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
    pub extra_currencies: HashMap<String, String>,
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
    pub block_ref: BlockId,
    #[serde(default)]
    pub now: u32,
    pub mc_block_seqno: u32,
    pub emulated: bool,
    pub finality: String,
    pub prev_trans_hash: String,
    pub prev_trans_lt: String,
    pub orig_status: String,
    pub end_status: String,
    pub total_fees: String,
    #[serde(default)]
    pub total_fees_extra_currencies: HashMap<String, String>,
    #[serde(default)]
    pub trace_external_hash: Option<String>,
    #[serde(default)]
    pub trace_id: Option<String>,
    #[serde(default)]
    pub child_transactions: Vec<String>,
    pub description: TransactionDescr,
    #[serde(default)]
    pub in_msg: Option<Message>,
    #[serde(default)]
    pub out_msgs: Vec<Message>,
    pub account_state_before: AccountState,
    pub account_state_after: AccountState,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountState {
    pub hash: String,
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
    #[serde(rename = "type")]
    pub kind: String,
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
    pub hash: String,
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
    pub mc_seqno_end: String,
    pub mc_seqno_start: String,
    pub start_lt: String,
    pub start_utime: u32,
    #[serde(default)]
    pub trace: Option<TraceNode>,
    pub trace_info: TraceInfo,
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
    fn wallet_dtos_accept_signed_int64_identifiers() {
        let information: V2WalletInformation = serde_json::from_value(serde_json::json!({
            "balance": "1",
            "seqno": -1,
            "wallet_id": 4_294_967_295_i64,
            "last_transaction_lt": "0",
            "last_transaction_hash": "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=",
            "status": "active"
        }))
        .expect("v3 wallet information uses upstream int64 fields");

        assert_eq!(information.seqno, Some(-1));
        assert_eq!(information.wallet_id, Some(4_294_967_295));

        let state: WalletState = serde_json::from_value(serde_json::json!({
            "address": "0:0000000000000000000000000000000000000000000000000000000000000000",
            "is_wallet": true,
            "seqno": -1,
            "wallet_id": 4_294_967_295_i64
        }))
        .expect("v3 wallet state uses upstream int64 fields");

        assert_eq!(state.seqno, Some(-1));
        assert_eq!(state.wallet_id, Some(4_294_967_295));
    }

    #[test]
    fn trace_accepts_additional_openapi_fields() {
        let trace: Trace = serde_json::from_value(serde_json::json!({
            "trace_id": "trace",
            "transactions_order": [],
            "transactions": {},
            "is_incomplete": false,
            "mc_seqno_start": "1",
            "mc_seqno_end": "1",
            "start_lt": "1",
            "start_utime": 1,
            "trace_info": {
                "transactions": 0,
                "messages": 0,
                "pending_messages": 0,
                "trace_state": "complete",
                "classification_state": "unclassified"
            },
            "warning": "upstream field not consumed by this response projection"
        }))
        .expect("v3 trace response must accept the full upstream envelope");

        assert_eq!(trace.trace_id, "trace");
    }

    #[test]
    fn traces_response_accepts_null_from_pending_traces() {
        let response: TracesResponse = serde_json::from_value(serde_json::json!({
            "traces": null,
            "address_book": {},
            "metadata": {}
        }))
        .expect("pendingTraces returns null instead of an empty OpenAPI array");

        assert!(response.traces.is_empty());
    }
}
