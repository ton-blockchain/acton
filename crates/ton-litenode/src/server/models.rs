use serde::Deserialize;
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
pub struct GetTransactionsRequest {
    pub address: String,
    #[serde(default = "default_limit")]
    pub limit: usize,
    pub lt: Option<u64>,
    pub hash: Option<String>,
    pub to_lt: Option<u64>,
}

pub const fn default_limit() -> usize {
    10
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
pub struct GetTracesQuery {
    pub hash: String,
}

#[derive(Deserialize)]
pub struct GetTransactionsBySourceRequest {
    pub source: String,
    pub limit: Option<usize>,
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
