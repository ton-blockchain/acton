use serde::{
    Deserialize, Deserializer, Serialize,
    de::{self, Error as _},
};
use serde_json::Value;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct EmptyRequest {}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcRequest<T> {
    pub jsonrpc: String,
    pub id: String,
    pub method: String,
    pub params: T,
}

impl<T> JsonRpcRequest<T> {
    pub fn new(id: impl Into<String>, method: impl Into<String>, params: T) -> Self {
        Self {
            jsonrpc: "2.0".to_owned(),
            id: id.into(),
            method: method.into(),
            params,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SendBocRequest {
    pub boc: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunGetMethodRequest {
    pub address: String,
    /// A method name or signed 32-bit method id.
    pub method: Value,
    pub stack: Vec<Value>,
    /// Historical masterchain seqno supported by `TonCenter` v2 and Acton localnet.
    pub seqno: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddressInformationRequest {
    pub address: String,
    pub seqno: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddressRequest {
    pub address: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetectHashRequest {
    pub hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LibrariesRequest {
    pub libraries: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionsRequest {
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
    #[serde(default)]
    #[serde(deserialize_with = "deserialize_optional_bool_from_wire")]
    pub archival: Option<bool>,
}

#[must_use]
pub const fn default_limit() -> usize {
    10
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TryLocateTxRequest {
    pub source: String,
    pub destination: String,
    pub created_lt: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigParamRequest {
    pub param: Option<i32>,
    pub config_id: Option<i32>,
    pub seqno: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigAllRequest {
    pub seqno: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockHeaderRequest {
    pub workchain: i32,
    pub shard: String,
    pub seqno: i32,
    pub root_hash: Option<String>,
    pub file_hash: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LookupBlockRequest {
    pub workchain: i32,
    pub shard: String,
    pub seqno: Option<i32>,
    pub lt: Option<u64>,
    pub unixtime: Option<u32>,
}

#[derive(Debug, Clone, Deserialize)]
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

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
enum BoolParam {
    Bool(bool),
    Number(i64),
    String(String),
}

fn deserialize_optional_bool_from_wire<'de, D>(deserializer: D) -> Result<Option<bool>, D::Error>
where
    D: Deserializer<'de>,
{
    Option::<BoolParam>::deserialize(deserializer)?
        .map(|value| match value {
            BoolParam::Bool(value) => Ok(value),
            BoolParam::Number(0) => Ok(false),
            BoolParam::Number(1) => Ok(true),
            BoolParam::Number(value) => Err(D::Error::custom(format!(
                "expected boolean integer 0 or 1, got `{value}`"
            ))),
            BoolParam::String(value) if value == "false" || value == "0" => Ok(false),
            BoolParam::String(value) if value == "true" || value == "1" => Ok(true),
            BoolParam::String(value) => Err(D::Error::custom(format!(
                "expected boolean string, got `{value}`"
            ))),
        })
        .transpose()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transactions_request_accepts_openapi_scalar_forms() {
        for value in [
            serde_json::json!({
                "address": "EQB0HzdrKy0awerTp1P3kgttmalQYfpbCiRKLg88SR5MUamv",
                "limit": 1,
                "lt": 8,
                "hash": "i91zjBL5M6ewi4wcL1JvqpQx/Um/hxHDXUD7FUDSq/I=",
                "to_lt": 2,
                "archival": true
            }),
            serde_json::json!({
                "address": "EQB0HzdrKy0awerTp1P3kgttmalQYfpbCiRKLg88SR5MUamv",
                "limit": "1",
                "lt": "8",
                "hash": "i91zjBL5M6ewi4wcL1JvqpQx/Um/hxHDXUD7FUDSq/I=",
                "to_lt": "2",
                "archival": "true"
            }),
        ] {
            let request: TransactionsRequest =
                serde_json::from_value(value).expect("OpenAPI scalar forms must parse");

            assert_eq!(request.limit, 1);
            assert_eq!(request.lt, Some(8));
            assert_eq!(request.to_lt, Some(2));
            assert_eq!(request.archival, Some(true));
        }
    }

    #[test]
    fn transactions_request_uses_default_limit() {
        let request: TransactionsRequest = serde_json::from_value(serde_json::json!({
            "address": "EQB0HzdrKy0awerTp1P3kgttmalQYfpbCiRKLg88SR5MUamv"
        }))
        .expect("request without explicit limit must parse");

        assert_eq!(request.limit, default_limit());
    }

    #[test]
    fn json_rpc_request_serializes_typed_params() {
        let request = JsonRpcRequest::new(
            "request-1",
            "sendBoc",
            SendBocRequest {
                boc: "te6ccgEBAQEAAgAAAA==".to_owned(),
            },
        );

        let value = serde_json::to_value(request).expect("request must serialize");
        assert_eq!(value["jsonrpc"], "2.0");
        assert_eq!(value["id"], "request-1");
        assert_eq!(value["method"], "sendBoc");
        assert_eq!(value["params"]["boc"], "te6ccgEBAQEAAgAAAA==");
    }
}
