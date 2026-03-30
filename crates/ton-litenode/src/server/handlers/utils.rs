use axum::Json;
use serde::de::DeserializeOwned;
use serde_json::Value;
use std::future::Future;
use std::time::{SystemTime, UNIX_EPOCH};

pub fn parse_params<T: DeserializeOwned>(params: Value, method: &str) -> anyhow::Result<T> {
    serde_json::from_value(params).map_err(|_| anyhow::anyhow!("Invalid params for {method}"))
}

pub fn parse_method_name(method: &Value) -> anyhow::Result<String> {
    match method {
        Value::String(s) => Ok(s.clone()),
        Value::Number(n) => Ok(n.to_string()),
        _ => anyhow::bail!("Invalid method format"),
    }
}

pub async fn handle_result<T, F>(
    result: impl Future<Output = anyhow::Result<T>>,
    mapper: F,
) -> Json<Value>
where
    F: FnOnce(&T) -> Value,
{
    match result.await {
        Ok(res) => Json(serde_json::json!({
            "ok": true,
            "result": mapper(&res),
            "@extra": get_extra()
        })),
        Err(e) => Json(serde_json::json!({
            "ok": false,
            "error": e.to_string(),
            "code": 500,
            "@extra": get_extra()
        })),
    }
}

#[must_use]
pub fn get_extra() -> String {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or_else(|_| "0".to_string(), |d| d.as_millis().to_string())
}
