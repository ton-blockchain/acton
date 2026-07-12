use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddressInformationQuery {
    pub address: String,
    pub use_v2: Option<bool>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AccountStatesQuery {
    pub address: Vec<String>,
    pub include_boc: Option<bool>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TracesQuery {
    #[serde(default)]
    pub account: Vec<String>,
    #[serde(default)]
    pub trace_id: Vec<String>,
    #[serde(default)]
    pub tx_hash: Vec<String>,
    #[serde(default)]
    pub msg_hash: Vec<String>,
    pub mc_seqno: Option<u32>,
    pub start_utime: Option<u32>,
    pub end_utime: Option<u32>,
    pub start_lt: Option<u64>,
    pub end_lt: Option<u64>,
    pub include_actions: Option<bool>,
    #[serde(default)]
    pub supported_action_types: Vec<String>,
    pub limit: Option<usize>,
    pub offset: Option<usize>,
    pub sort: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TransactionsQuery {
    pub workchain: Option<i32>,
    pub shard: Option<String>,
    pub seqno: Option<u32>,
    pub mc_seqno: Option<u32>,
    #[serde(default)]
    pub account: Vec<String>,
    #[serde(default)]
    pub exclude_account: Vec<String>,
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

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BlocksQuery {
    pub workchain: Option<i32>,
    pub shard: Option<String>,
    pub seqno: Option<u32>,
    pub root_hash: Option<String>,
    pub file_hash: Option<String>,
    pub mc_seqno: Option<u32>,
    pub start_utime: Option<u32>,
    pub end_utime: Option<u32>,
    pub start_lt: Option<u64>,
    pub end_lt: Option<u64>,
    pub limit: Option<usize>,
    pub offset: Option<usize>,
    pub sort: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TransactionsByMessageQuery {
    pub msg_hash: Option<String>,
    pub body_hash: Option<String>,
    pub opcode: Option<String>,
    pub direction: Option<String>,
    pub limit: Option<usize>,
    pub offset: Option<usize>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PendingTransactionsQuery {
    /// `TonCenter` currently requires at least one account even though its Swagger marks it optional.
    #[serde(default)]
    pub account: Vec<String>,
    #[serde(default)]
    pub trace_id: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct JettonMastersQuery {
    #[serde(default)]
    pub address: Vec<String>,
    #[serde(default)]
    pub admin_address: Vec<String>,
    pub limit: Option<usize>,
    pub offset: Option<usize>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct JettonWalletsQuery {
    #[serde(default)]
    pub address: Vec<String>,
    #[serde(default)]
    pub owner_address: Vec<String>,
    #[serde(default)]
    pub jetton_address: Vec<String>,
    pub exclude_zero_balance: Option<bool>,
    pub limit: Option<usize>,
    pub offset: Option<usize>,
    pub sort: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct NftItemsQuery {
    #[serde(default)]
    pub address: Vec<String>,
    #[serde(default)]
    pub owner_address: Vec<String>,
    #[serde(default)]
    pub collection_address: Vec<String>,
    #[serde(default)]
    pub index: Vec<String>,
    pub include_on_sale: Option<bool>,
    pub sort_by_last_transaction_lt: Option<bool>,
    pub limit: Option<usize>,
    pub offset: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SendMessageRequest {
    pub boc: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunGetMethodRequest {
    pub address: String,
    pub method: String,
    pub stack: Vec<Value>,
}
