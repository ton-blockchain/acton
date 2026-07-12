use serde::{Deserialize, Deserializer, Serialize, de::Error as _};
use serde_json::Value;

use super::StringOrNumber;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct EmptyRequest {}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcRequest<T> {
    pub jsonrpc: String,
    pub id: StringOrNumber,
    pub method: String,
    pub params: T,
}

impl<T> JsonRpcRequest<T> {
    pub fn new(id: impl Into<String>, method: impl Into<String>, params: T) -> Self {
        Self {
            jsonrpc: "2.0".to_owned(),
            id: StringOrNumber::String(id.into()),
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
    pub method: StringOrNumber,
    pub stack: Vec<Value>,
    /// Historical masterchain seqno supported by `TonCenter` v2 and Acton localnet.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seqno: Option<StringOrNumber>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddressInformationRequest {
    pub address: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seqno: Option<StringOrNumber>,
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
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub libraries: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionsRequest {
    pub address: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<StringOrNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lt: Option<StringOrNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hash: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub to_lt: Option<StringOrNumber>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "deserialize_optional_bool_from_wire"
    )]
    pub archival: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TryLocateTxRequest {
    pub source: String,
    pub destination: String,
    pub created_lt: StringOrNumber,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigParamRequest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub param: Option<StringOrNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub config_id: Option<StringOrNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seqno: Option<StringOrNumber>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigAllRequest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seqno: Option<StringOrNumber>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockHeaderRequest {
    pub workchain: StringOrNumber,
    pub shard: StringOrNumber,
    pub seqno: StringOrNumber,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub root_hash: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file_hash: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockTransactionsRequest {
    pub workchain: StringOrNumber,
    pub shard: StringOrNumber,
    pub seqno: StringOrNumber,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub root_hash: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file_hash: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after_lt: Option<StringOrNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after_hash: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub count: Option<StringOrNumber>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SeqnoRequest {
    pub seqno: StringOrNumber,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LookupBlockRequest {
    pub workchain: StringOrNumber,
    pub shard: StringOrNumber,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seqno: Option<StringOrNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lt: Option<StringOrNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unixtime: Option<StringOrNumber>,
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

            assert_eq!(request.limit.as_ref().unwrap().to_usize().unwrap(), 1);
            assert_eq!(request.lt.as_ref().unwrap().to_u64().unwrap(), 8);
            assert_eq!(request.to_lt.as_ref().unwrap().to_u64().unwrap(), 2);
            assert_eq!(request.archival, Some(true));
        }
    }

    #[test]
    fn transactions_request_uses_default_limit() {
        let request: TransactionsRequest = serde_json::from_value(serde_json::json!({
            "address": "EQB0HzdrKy0awerTp1P3kgttmalQYfpbCiRKLg88SR5MUamv"
        }))
        .expect("request without explicit limit must parse");

        assert!(request.limit.is_none());
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
