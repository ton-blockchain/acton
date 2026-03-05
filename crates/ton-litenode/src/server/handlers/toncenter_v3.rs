use super::utils::{get_extra, handle_result, parse_method_name};
use crate::api::toncenter_v3;
use crate::litenode::LiteNode;
use crate::server::models::{
    GetAddressInformationV3Request, GetJettonMastersRequest, GetJettonWalletsRequest,
    GetTracesQuery, RunGetMethodRequest, SendBocRequest,
};
use axum::{
    Json,
    extract::{Query, State},
};
use serde_json::Value;
use serde_json::json;
use std::sync::Arc;
use toncenter_v3 as v3;

pub async fn get_traces(
    State(node): State<Arc<LiteNode>>,
    Query(payload): Query<GetTracesQuery>,
) -> Json<Value> {
    handle_result(node.get_traces(payload.hash), v3::map_traces).await
}

pub async fn get_address_information_v3(
    State(node): State<Arc<LiteNode>>,
    Query(payload): Query<GetAddressInformationV3Request>,
) -> Json<Value> {
    let _use_v2 = payload.use_v2.unwrap_or(true);

    handle_result(
        node.get_address_information(payload.address, None),
        toncenter_v3::map_address_information,
    )
    .await
}

pub async fn get_jetton_masters(
    State(node): State<Arc<LiteNode>>,
    Query(payload): Query<GetJettonMastersRequest>,
) -> Json<Value> {
    handle_result(
        node.get_jetton_masters(
            payload.address,
            payload.admin_address,
            payload.limit,
            payload.offset,
        ),
        v3::map_jetton_masters,
    )
    .await
}

pub async fn get_jetton_wallets(
    State(node): State<Arc<LiteNode>>,
    Query(payload): Query<GetJettonWalletsRequest>,
) -> Json<Value> {
    handle_result(
        node.get_jetton_wallets(
            payload.address,
            payload.owner_address,
            payload.jetton_address,
            payload.exclude_zero_balance,
            payload.limit,
            payload.offset,
        ),
        v3::map_jetton_wallets,
    )
    .await
}

pub async fn send_message_v3(
    State(node): State<Arc<LiteNode>>,
    Json(payload): Json<SendBocRequest>,
) -> Json<Value> {
    handle_result(node.send_boc(payload.boc), toncenter_v3::map_send_message).await
}

pub async fn run_get_method_v3(
    State(node): State<Arc<LiteNode>>,
    Json(payload): Json<RunGetMethodRequest>,
) -> Json<Value> {
    let method_str = match parse_method_name(&payload.method) {
        Ok(s) => s,
        Err(e) => {
            return Json(json!({
                "ok": false,
                "error": e.to_string(),
                "code": 400,
                "@extra": get_extra()
            }));
        }
    };

    let stack = match normalize_v3_stack(payload.stack) {
        Ok(stack) => stack,
        Err(e) => {
            return Json(json!({
                "ok": false,
                "error": e.to_string(),
                "code": 400,
                "@extra": get_extra()
            }));
        }
    };

    handle_result(
        node.run_get_method(payload.address, method_str, stack, payload.seqno),
        toncenter_v3::map_run_get_method_v3,
    )
    .await
}

fn normalize_v3_stack(stack: Vec<Value>) -> anyhow::Result<Vec<Value>> {
    stack.into_iter().map(normalize_v3_stack_item).collect()
}

fn normalize_v3_stack_item(item: Value) -> anyhow::Result<Value> {
    if item.is_array() {
        return Ok(item);
    }

    let stack_type = item
        .get("type")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow::anyhow!("v3 stack entry must contain string `type`"))?;
    let value = item.get("value").cloned().unwrap_or(Value::Null);

    match stack_type {
        "null" => Ok(json!(["null", Value::Null])),
        "num" => Ok(json!(["num", value])),
        "cell" | "slice" | "builder" => {
            let bytes = extract_stack_bytes(&value, stack_type)?;
            Ok(json!([stack_type, { "bytes": bytes }]))
        }
        "tuple" | "list" => {
            let elements = value
                .as_array()
                .ok_or_else(|| anyhow::anyhow!("{stack_type} stack value must be an array"))?
                .iter()
                .cloned()
                .map(normalize_v3_stack_item)
                .collect::<anyhow::Result<Vec<_>>>()?;
            Ok(json!([stack_type, { "elements": elements }]))
        }
        _ => anyhow::bail!("Unsupported v3 stack entry type: {stack_type}"),
    }
}

fn extract_stack_bytes(value: &Value, stack_type: &str) -> anyhow::Result<String> {
    if let Some(b64) = value.as_str() {
        return Ok(b64.to_owned());
    }
    if let Some(b64) = value.get("bytes").and_then(Value::as_str) {
        return Ok(b64.to_owned());
    }
    anyhow::bail!("{stack_type} stack value must be a base64 string or an object with `bytes`")
}
