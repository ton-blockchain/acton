use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Deserialize)]
pub struct JsonRpcRequest {
    #[serde(rename = "jsonrpc")]
    pub _jsonrpc: String,
    pub id: Value,
    pub method: String,
    pub params: Value,
}

#[derive(Deserialize)]
pub struct SendBocRequest {
    pub boc: String,
}

#[derive(Deserialize)]
pub struct RunGetMethodRequest {
    pub address: String,
    pub method: Value, // String or Integer
    pub stack: Vec<Value>,
    pub seqno: Option<u32>,
}

#[derive(Deserialize)]
pub struct GetAddressInformationRequest {
    pub address: String,
    pub seqno: Option<u32>,
}

#[derive(Deserialize)]
pub struct GetAddressInformationV3Request {
    pub address: String,
    pub use_v2: Option<bool>,
}

#[derive(Deserialize)]
pub struct GetAccountStatesV3Request {
    pub address: Option<Vec<String>>,
    pub include_boc: Option<bool>,
}

#[derive(Deserialize)]
pub struct AddressRequest {
    pub address: String,
}

#[derive(Deserialize)]
pub struct DetectHashRequest {
    pub hash: String,
}

#[derive(Deserialize)]
pub struct GetLibrariesRequest {
    pub libraries: String,
}

#[derive(Deserialize)]
pub struct GetTransactionsRequest {
    pub address: String,
    #[serde(default = "default_limit")]
    pub limit: usize,
    pub lt: Option<u64>,
    pub hash: Option<String>,
    pub to_lt: Option<u64>,
}

#[must_use]
pub const fn default_limit() -> usize {
    10
}

#[derive(Deserialize)]
pub struct TryLocateTxRequest {
    pub source: String,
    pub destination: String,
    pub created_lt: u64,
}

#[derive(Deserialize)]
pub struct GetConfigParamRequest {
    pub param: Option<i32>,
    pub config_id: Option<i32>,
    pub seqno: Option<i32>,
}

#[derive(Deserialize)]
pub struct GetConfigAllRequest {
    pub seqno: Option<i32>,
}

#[derive(Deserialize)]
pub struct GetBlockRequest {
    /// Workchain index (ignored, dev node only uses workchain 0)
    #[allow(dead_code)]
    pub workchain: Option<i32>,
    /// Shard ID (ignored, dev node only uses shard -9223372036854775808)
    #[allow(dead_code)]
    pub shard: Option<String>,
    pub seqno: i32,
}

#[derive(Deserialize)]
pub struct LookupBlockRequest {
    pub workchain: i32,
    pub shard: String,
    pub seqno: Option<i32>,
    pub lt: Option<u64>,
    pub unixtime: Option<u32>,
}

#[derive(Deserialize)]
pub struct FaucetRequest {
    pub address: String,
    pub amount: u128,
}

#[derive(Deserialize)]
pub struct StatePathRequest {
    pub path: String,
}

#[derive(Deserialize)]
pub struct SetShardAccountRequest {
    pub address: String,
    pub shard_account: String,
}

#[derive(Deserialize)]
pub struct GetTracesQuery {
    #[serde(alias = "hash")]
    pub tx_hash: Option<String>,
    pub msg_hash: Option<String>,
}

#[derive(Deserialize)]
pub struct GetTransactionsV3Query {
    pub workchain: Option<i32>,
    pub shard: Option<String>,
    pub seqno: Option<u32>,
    pub mc_seqno: Option<u32>,
    pub account: Option<String>,
    pub exclude_account: Option<String>,
    pub hash: Option<String>,
    pub lt: Option<u64>,
    pub start_utime: Option<u32>,
    pub end_utime: Option<u32>,
    pub start_lt: Option<u64>,
    pub end_lt: Option<u64>,
    pub limit: Option<usize>,
    pub offset: Option<usize>,
    pub sort: Option<String>,
}

#[derive(Deserialize)]
pub struct GetTransactionsByMessageV3Query {
    pub msg_hash: Option<String>,
    pub body_hash: Option<String>,
    pub opcode: Option<String>,
    pub direction: Option<String>,
    pub limit: Option<usize>,
    pub offset: Option<usize>,
}

#[derive(Deserialize)]
pub struct GetPendingTransactionsV3Query {
    pub account: Option<String>,
    pub trace_id: Option<String>,
}

#[derive(Deserialize)]
pub struct EmulateTraceRequest {
    pub boc: Option<String>,
    pub ignore_chksig: Option<bool>,
    pub include_code_data: Option<bool>,
    pub include_address_book: Option<bool>,
    pub include_metadata: Option<bool>,
    pub with_actions: Option<bool>,
    pub mc_block_seqno: Option<u32>,
}

#[derive(Deserialize)]
pub struct SetAddressNameRequest {
    pub address: String,
    pub name: String,
}

#[derive(Deserialize)]
pub struct GetAddressNameQuery {
    pub address: String,
}

#[derive(Deserialize)]
pub struct CompilerAbiRegistration {
    pub code_hash: String,
    pub compiler_abi: Value,
}

#[derive(Deserialize)]
pub struct RegisterCompilerAbisRequest {
    pub entries: Vec<CompilerAbiRegistration>,
}

#[derive(Deserialize)]
pub struct GetCompilerAbiQuery {
    pub code_hash: String,
}

#[derive(Deserialize)]
pub struct GetJettonMastersRequest {
    pub address: Option<String>,
    pub admin_address: Option<String>,
    pub limit: Option<usize>,
    pub offset: Option<usize>,
}

#[derive(Deserialize, Serialize)]
pub struct GetJettonWalletsRequest {
    pub address: Option<String>,
    pub owner_address: Option<String>,
    pub jetton_address: Option<String>,
    pub exclude_zero_balance: Option<bool>,
    pub limit: Option<usize>,
    pub offset: Option<usize>,
}

#[derive(Deserialize, Serialize)]
pub struct GetNftItemsRequest {
    pub address: Option<String>,
    pub owner_address: Option<String>,
    pub collection_address: Option<String>,
    pub index: Option<String>,
    pub include_on_sale: Option<bool>,
    pub sort_by_last_transaction_lt: Option<bool>,
    pub limit: Option<usize>,
    pub offset: Option<usize>,
}
