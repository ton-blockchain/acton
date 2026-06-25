use serde::{
    Deserialize, Deserializer, Serialize,
    de::{self, Error as _},
};
use serde_json::Value;
use std::collections::BTreeMap;

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
pub struct GetVerifiedSourceRequest {
    pub address: Option<String>,
    pub code_hash: Option<String>,
}

#[derive(Deserialize)]
pub struct BuildSourceTraceRequest {
    pub vm_logs: String,
    pub code_hash: String,
    pub source_bundle: SourceTraceBundleRequest,
}

#[derive(Deserialize)]
pub struct SourceTraceBundleRequest {
    pub source_bundle_hash: String,
    pub language: String,
    pub compiler_version: String,
    pub entrypoint: String,
    pub compile_params: Value,
    pub files: Vec<SourceTraceFileRequest>,
}

#[derive(Deserialize)]
pub struct SourceTraceFileRequest {
    pub path: String,
    pub content_base64: String,
    pub content_text: Option<String>,
}

impl SourceTraceBundleRequest {
    #[must_use]
    pub fn import_mappings(&self) -> Option<BTreeMap<String, String>> {
        self.compile_params
            .get("import_mappings")
            .and_then(Value::as_object)
            .map(|mappings| {
                mappings
                    .iter()
                    .filter_map(|(key, value)| {
                        value.as_str().map(|value| (key.clone(), value.to_owned()))
                    })
                    .collect()
            })
    }
}

#[derive(Deserialize)]
pub struct GetTransactionsRequest {
    pub address: String,
    #[serde(default = "default_limit")]
    #[serde(deserialize_with = "deserialize_usize_from_string_or_number")]
    pub limit: usize,
    #[serde(default)]
    #[serde(deserialize_with = "deserialize_optional_u64_from_string_or_number")]
    pub lt: Option<u64>,
    pub hash: Option<String>,
    #[serde(default)]
    #[serde(deserialize_with = "deserialize_optional_u64_from_string_or_number")]
    pub to_lt: Option<u64>,
}

#[must_use]
pub const fn default_limit() -> usize {
    10
}

#[derive(Deserialize)]
#[serde(untagged)]
enum NumberParam {
    Number(u64),
    String(String),
}

fn deserialize_optional_u64_from_string_or_number<'de, D>(
    deserializer: D,
) -> Result<Option<u64>, D::Error>
where
    D: Deserializer<'de>,
{
    Option::<NumberParam>::deserialize(deserializer)?
        .map(parse_number_param)
        .transpose()
}

fn deserialize_usize_from_string_or_number<'de, D>(deserializer: D) -> Result<usize, D::Error>
where
    D: Deserializer<'de>,
{
    let value = parse_number_param(NumberParam::deserialize(deserializer)?)?;
    usize::try_from(value).map_err(|_| D::Error::custom("value does not fit into usize"))
}

fn parse_number_param<E>(param: NumberParam) -> Result<u64, E>
where
    E: de::Error,
{
    match param {
        NumberParam::Number(value) => Ok(value),
        NumberParam::String(value) => value
            .parse()
            .map_err(|_| E::custom(format!("expected unsigned integer string, got `{value}`"))),
    }
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
pub struct ChangeAccountStateRequest {
    pub address: String,
    pub state: ChangeAccountStatePayload,
    #[serde(default = "default_true")]
    pub mine: bool,
}

const fn default_true() -> bool {
    true
}

#[derive(Deserialize)]
#[serde(tag = "type")]
pub enum ChangeAccountStatePayload {
    #[serde(rename = "nonexist")]
    Nonexist,
    #[serde(rename = "uninit")]
    Uninit { balance: Option<String> },
    #[serde(rename = "frozen")]
    Frozen {
        source: Option<String>,
        frozen_hash: Option<String>,
        balance: Option<String>,
    },
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
pub struct GetBlocksV3Query {
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
pub struct SetNetworkConditionsRequest {
    pub response_delay_ms: u64,
}

#[derive(Default, Deserialize)]
pub struct MineBlocksRequest {
    pub blocks: Option<u32>,
}

#[derive(Deserialize)]
pub struct SetMiningModeRequest {
    pub skip_empty_blocks: bool,
}

#[derive(Deserialize)]
pub struct CreateRecoveryPointRequest {
    pub name: String,
    #[serde(default)]
    pub force: bool,
}

#[derive(Deserialize)]
pub struct RevertRecoveryPointRequest {
    pub name: String,
}

#[derive(Deserialize)]
pub struct ExportRecoveryPointRequest {
    pub name: String,
    pub path: String,
}

#[derive(Deserialize)]
pub struct ImportRecoveryPointRequest {
    pub name: String,
    pub path: String,
    #[serde(default)]
    pub force: bool,
}

#[derive(Deserialize)]
pub struct IncreaseTimeRequest {
    pub seconds: u64,
}

#[derive(Deserialize)]
pub struct SetTimeRequest {
    pub timestamp: u32,
}

#[derive(Deserialize)]
pub struct SetNextBlockTimestampRequest {
    pub timestamp: u32,
}

#[derive(Deserialize)]
pub struct GetApiCallsRequest {
    pub limit: Option<usize>,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn get_transactions_request_accepts_numeric_cursor_params() {
        let request: GetTransactionsRequest = serde_json::from_value(serde_json::json!({
            "address": "EQB0HzdrKy0awerTp1P3kgttmalQYfpbCiRKLg88SR5MUamv",
            "limit": 1,
            "lt": 8,
            "hash": "i91zjBL5M6ewi4wcL1JvqpQx/Um/hxHDXUD7FUDSq/I=",
            "to_lt": 2,
            "archival": true
        }))
        .expect("numeric transaction cursor params must parse");

        assert_eq!(request.limit, 1);
        assert_eq!(request.lt, Some(8));
        assert_eq!(
            request.hash.as_deref(),
            Some("i91zjBL5M6ewi4wcL1JvqpQx/Um/hxHDXUD7FUDSq/I=")
        );
        assert_eq!(request.to_lt, Some(2));
    }

    #[test]
    fn get_transactions_request_accepts_string_cursor_params() {
        let request: GetTransactionsRequest = serde_json::from_value(serde_json::json!({
            "address": "EQB0HzdrKy0awerTp1P3kgttmalQYfpbCiRKLg88SR5MUamv",
            "limit": "1",
            "lt": "8",
            "hash": "i91zjBL5M6ewi4wcL1JvqpQx/Um/hxHDXUD7FUDSq/I=",
            "to_lt": "2",
            "archival": true
        }))
        .expect("string transaction cursor params must parse");

        assert_eq!(request.limit, 1);
        assert_eq!(request.lt, Some(8));
        assert_eq!(
            request.hash.as_deref(),
            Some("i91zjBL5M6ewi4wcL1JvqpQx/Um/hxHDXUD7FUDSq/I=")
        );
        assert_eq!(request.to_lt, Some(2));
    }

    #[test]
    fn get_transactions_request_uses_default_limit() {
        let request: GetTransactionsRequest = serde_json::from_value(serde_json::json!({
            "address": "EQB0HzdrKy0awerTp1P3kgttmalQYfpbCiRKLg88SR5MUamv"
        }))
        .expect("request without explicit limit must parse");

        assert_eq!(request.limit, default_limit());
    }
}
