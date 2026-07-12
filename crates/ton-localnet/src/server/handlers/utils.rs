use axum::Json;
use axum::response::{IntoResponse, Response};
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::Value;
use std::future::Future;
use std::time::{SystemTime, UNIX_EPOCH};
use ton_api::toncenter::v2::StringOrNumber;

pub fn parse_params<T: DeserializeOwned>(params: Value, method: &str) -> anyhow::Result<T> {
    serde_json::from_value(params).map_err(|_| anyhow::anyhow!("Invalid params for {method}"))
}

pub fn parse_method_name(method: &StringOrNumber) -> anyhow::Result<String> {
    match method {
        StringOrNumber::String(value) => Ok(value.clone()),
        StringOrNumber::Number(value) => Ok(value.to_string()),
        StringOrNumber::Unsigned(value) => Ok(value.to_string()),
    }
}

pub async fn handle_result<T, F, M>(
    result: impl Future<Output = anyhow::Result<T>>,
    mapper: F,
) -> Response
where
    F: FnOnce(&T) -> M,
    M: Serialize,
{
    match result.await {
        Ok(res) => Json(ton_api::toncenter::v2::TonlibResponse {
            ok: true,
            result: mapper(&res),
            extra: Some(get_extra()),
        })
        .into_response(),
        Err(e) => Json(ton_api::toncenter::v2::TonlibErrorResponse {
            ok: false,
            error: e.to_string(),
            code: 500,
            extra: Some(get_extra()),
            jsonrpc: None,
            id: None,
        })
        .into_response(),
    }
}

#[must_use]
pub fn get_extra() -> String {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or_else(|_| "0".to_string(), |d| d.as_millis().to_string())
}
