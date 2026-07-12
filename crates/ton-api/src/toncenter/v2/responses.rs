use super::StringOrNumber;
use anyhow::Context as _;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tvm_ffi::json_stack::{json_to_legacy_stack, json_to_stack};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TonlibResponse<T> {
    pub ok: bool,
    pub result: T,
    #[serde(default, rename = "@extra")]
    pub extra: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcResponse<T> {
    /// `TonCenter` accepts JSON-RPC requests but may omit JSON-RPC metadata in responses.
    #[serde(default)]
    pub jsonrpc: Option<String>,
    #[serde(default)]
    pub id: Option<Value>,
    #[serde(flatten)]
    pub response: TonlibResponse<T>,
}

impl<T> JsonRpcResponse<T> {
    pub fn into_result(self) -> T {
        self.response.result
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TonBlockIdExt {
    #[serde(rename = "@type")]
    pub type_field: String,
    pub workchain: i32,
    pub shard: String,
    pub seqno: u64,
    pub root_hash: String,
    pub file_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MasterchainInfo {
    #[serde(rename = "@type")]
    pub type_field: String,
    pub last: TonBlockIdExt,
    pub state_root_hash: String,
    pub init: TonBlockIdExt,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetectAddressBase64Variant {
    #[serde(rename = "@type")]
    pub type_field: String,
    pub b64: String,
    pub b64url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetectAddress {
    #[serde(rename = "@type")]
    pub type_field: String,
    pub raw_form: String,
    pub bounceable: DetectAddressBase64Variant,
    pub non_bounceable: DetectAddressBase64Variant,
    pub given_type: String,
    pub test_only: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetectHash {
    #[serde(rename = "@type")]
    pub type_field: String,
    pub b64: String,
    pub b64url: String,
    pub hex: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockHeader {
    #[serde(rename = "@type")]
    pub type_field: String,
    pub id: TonBlockIdExt,
    pub global_id: i32,
    pub version: i32,
    pub after_merge: bool,
    pub after_split: bool,
    pub before_split: bool,
    pub want_merge: bool,
    pub want_split: bool,
    pub validator_list_hash_short: i32,
    pub catchain_seqno: i32,
    pub min_ref_mc_seqno: i32,
    pub is_key_block: bool,
    pub prev_key_block_seqno: i32,
    pub start_lt: String,
    pub end_lt: String,
    pub gen_utime: u32,
    pub prev_blocks: Vec<TonBlockIdExt>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResultOk {
    #[serde(rename = "@type")]
    pub type_field: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtMessageInfo {
    #[serde(rename = "@type")]
    pub type_field: String,
    pub hash: String,
    pub hash_norm: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TvmCell {
    #[serde(rename = "@type")]
    pub type_field: String,
    pub bytes: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigInfo {
    #[serde(rename = "@type")]
    pub type_field: String,
    pub config: TvmCell,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LibraryResult {
    #[serde(rename = "@type")]
    pub type_field: String,
    pub result: Vec<LibraryEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LibraryEntry {
    #[serde(rename = "@type")]
    pub type_field: String,
    pub hash: String,
    pub data: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddressInformation {
    #[serde(rename = "@type")]
    pub type_field: String,
    pub balance: StringOrNumber,
    pub extra_currencies: Vec<ExtraCurrencyBalance>,
    pub code: String,
    pub data: String,
    pub state: String,
    pub frozen_hash: String,
    pub last_transaction_id: InternalTransactionId,
    pub block_id: TonBlockIdExt,
    pub sync_utime: u64,
    #[serde(default)]
    pub suspended: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtraCurrencyBalance {
    #[serde(rename = "@type")]
    pub type_field: String,
    pub id: i32,
    pub amount: StringOrNumber,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InternalTransactionId {
    #[serde(rename = "@type")]
    pub type_field: String,
    pub lt: String,
    pub hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunGetMethodResult {
    #[serde(rename = "@type")]
    pub type_field: String,
    pub gas_used: i64,
    pub stack: Vec<Value>,
    pub exit_code: i32,
    pub block_id: TonBlockIdExt,
    pub last_transaction_id: InternalTransactionId,
}

impl RunGetMethodResult {
    pub fn parse_stack_tuple(&self) -> anyhow::Result<tvm_ffi::stack::Tuple> {
        match json_to_legacy_stack(self.stack.clone()) {
            Ok(tuple) => Ok(tuple),
            Err(legacy_err) => json_to_stack(self.stack.clone()).with_context(|| {
                format!(
                    "Failed to parse stack as legacy and std formats. Legacy error: {legacy_err}"
                )
            }),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Transaction {
    #[serde(rename = "@type")]
    pub type_field: String,
    pub utime: u64,
    pub data: String,
    pub transaction_id: InternalTransactionId,
    pub fee: String,
    pub storage_fee: String,
    pub other_fee: String,
    pub in_msg: Option<Message>,
    pub out_msgs: Vec<Message>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    #[serde(rename = "@type")]
    pub type_field: String,
    pub source: Option<String>,
    pub destination: Option<String>,
    pub value: String,
    pub fwd_fee: Option<String>,
    pub ihr_fee: Option<String>,
    pub created_lt: Option<String>,
    pub hash: Option<String>,
    pub body_hash: Option<String>,
    pub message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TonlibErrorResponse {
    pub ok: bool,
    pub error: String,
    pub code: i32,
    #[serde(default, rename = "@extra")]
    pub extra: Option<String>,
    #[serde(default)]
    pub jsonrpc: Option<String>,
    #[serde(default)]
    pub id: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_rpc_response_deserializes_generic_result() {
        let response: JsonRpcResponse<String> = serde_json::from_value(serde_json::json!({
            "jsonrpc": "2.0",
            "id": "request-1",
            "ok": true,
            "result": "active",
            "@extra": "metadata"
        }))
        .expect("generic response must deserialize");

        assert_eq!(response.into_result(), "active");
    }
}
