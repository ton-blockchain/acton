use super::models::*;
use crate::api::toncenter_v3;
use crate::litenode::LiteNode;
use crate::{api, node};
use axum::{
    Json,
    extract::{Query, State},
    http::StatusCode,
    response::IntoResponse,
};
use serde_json::Value;
use std::future::Future;
use std::sync::Arc;

pub async fn json_rpc(
    State(node): State<Arc<LiteNode>>,
    Json(payload): Json<JsonRpcRequest>,
) -> impl IntoResponse {
    tracing::debug!(
        "JSON-RPC request: method={}, id={:?}",
        payload.method,
        payload.id
    );
    let result = match payload.method.as_str() {
        "sendBoc" => {
            if let Ok(req) = serde_json::from_value::<SendBocRequest>(payload.params) {
                node.send_boc(req.boc)
                    .await
                    .map(|r| api::map_block_transactions(&r))
            } else {
                Err(anyhow::anyhow!("Invalid params for sendBoc"))
            }
        }
        "sendBocReturnHash" => {
            if let Ok(req) = serde_json::from_value::<SendBocRequest>(payload.params) {
                node.send_boc(req.boc)
                    .await
                    .map(|r| api::map_send_boc_return_hash(&r))
            } else {
                Err(anyhow::anyhow!("Invalid params for sendBocReturnHash"))
            }
        }
        "runGetMethod" => {
            if let Ok(req) = serde_json::from_value::<RunGetMethodRequest>(payload.params) {
                let method_str = match req.method {
                    Value::String(s) => s,
                    Value::Number(n) => n.to_string(),
                    _ => {
                        return (
                            StatusCode::BAD_REQUEST,
                            Json(serde_json::json!({
                                "jsonrpc": "2.0",
                                "id": payload.id,
                                "ok": false,
                                "error": "Invalid method format",
                                "code": 400
                            })),
                        )
                            .into_response();
                    }
                };
                node.run_get_method(req.address, method_str, req.stack, req.seqno)
                    .await
                    .map(|r| api::map_run_get_method(&r, true))
            } else {
                Err(anyhow::anyhow!("Invalid params for runGetMethod"))
            }
        }
        "runGetMethodStd" => {
            if let Ok(req) = serde_json::from_value::<RunGetMethodRequest>(payload.params) {
                let method_str = match req.method {
                    Value::String(s) => s,
                    Value::Number(n) => n.to_string(),
                    _ => {
                        return (
                            StatusCode::BAD_REQUEST,
                            Json(serde_json::json!({
                                "jsonrpc": "2.0",
                                "id": payload.id,
                                "ok": false,
                                "error": "Invalid method format",
                                "code": 400
                            })),
                        )
                            .into_response();
                    }
                };
                node.run_get_method(req.address, method_str, req.stack, req.seqno)
                    .await
                    .map(|r| api::map_run_get_method(&r, false))
            } else {
                Err(anyhow::anyhow!("Invalid params for runGetMethodStd"))
            }
        }
        "getAddressInformation" => {
            if let Ok(req) = serde_json::from_value::<GetAddressInformationRequest>(payload.params)
            {
                node.get_address_information(req.address, req.seqno)
                    .await
                    .map(|r| api::map_account_state(&r))
            } else {
                Err(anyhow::anyhow!("Invalid params for getAddressInformation"))
            }
        }
        "getAddressBalance" => {
            if let Ok(req) = serde_json::from_value::<GetAddressInformationRequest>(payload.params)
            {
                node.get_address_balance(req.address, req.seqno)
                    .await
                    .map(|r| serde_json::json!({ "ok": true, "result": r.to_string() }))
            } else {
                Err(anyhow::anyhow!("Invalid params for getAddressBalance"))
            }
        }
        "getAddressState" => {
            if let Ok(req) = serde_json::from_value::<GetAddressInformationRequest>(payload.params)
            {
                node.get_address_state(req.address, req.seqno).await.map(|r| serde_json::json!({ "ok": true, "result": format!("{:?}", r).to_lowercase() }))
            } else {
                Err(anyhow::anyhow!("Invalid params for getAddressState"))
            }
        }
        "getExtendedAddressInformation" => {
            if let Ok(req) = serde_json::from_value::<GetAddressInformationRequest>(payload.params)
            {
                node.get_address_information(req.address, req.seqno)
                    .await
                    .map(|r| api::map_extended_account_state(&r))
            } else {
                Err(anyhow::anyhow!(
                    "Invalid params for getExtendedAddressInformation"
                ))
            }
        }
        "getTransactions" => {
            if let Ok(req) = serde_json::from_value::<GetTransactionsRequest>(payload.params) {
                node.get_transactions(req.address, req.limit, req.lt, req.hash, req.to_lt)
                    .await.map(|r| serde_json::json!({ "ok": true, "result": r.iter().map(api::map_transaction).collect::<Vec<_>>() }))
            } else {
                Err(anyhow::anyhow!("Invalid params for getTransactions"))
            }
        }
        "getBlockHeader" => {
            if let Ok(req) = serde_json::from_value::<GetBlockRequest>(payload.params) {
                node.get_block_header(req.seqno as u32)
                    .await
                    .map(|r| api::map_block_header(&r))
            } else {
                Err(anyhow::anyhow!("Invalid params for getBlockHeader"))
            }
        }
        "getBlockTransactions" => {
            if let Ok(req) = serde_json::from_value::<GetBlockRequest>(payload.params) {
                node.get_block_transactions(req.seqno as u32)
                    .await
                    .map(|r| api::map_block_transactions(&r))
            } else {
                Err(anyhow::anyhow!("Invalid params for getBlockTransactions"))
            }
        }
        "getBlockTransactionsExt" => {
            if let Ok(req) = serde_json::from_value::<GetBlockRequest>(payload.params) {
                node.get_block_transactions(req.seqno as u32)
                    .await
                    .map(|r| api::map_block_transactions_ext(&r))
            } else {
                Err(anyhow::anyhow!(
                    "Invalid params for getBlockTransactionsExt"
                ))
            }
        }
        "getMasterchainInfo" => node
            .get_masterchain_info()
            .await
            .map(|r| api::map_masterchain_info(&r)),
        "shards" => {
            if let Ok(req) = serde_json::from_value::<GetBlockRequest>(payload.params) {
                node.get_shards(req.seqno as u32).await.map(|r| {
                    serde_json::json!({
                        "ok": true,
                        "result": {
                            "@type": "blocks.shards",
                            "shards": r.iter().map(api::map_block_id).collect::<Vec<_>>()
                        }
                    })
                })
            } else {
                Err(anyhow::anyhow!("Invalid params for shards"))
            }
        }
        "lookupBlock" => {
            if let Ok(req) = serde_json::from_value::<LookupBlockRequest>(payload.params) {
                node.lookup_block(
                    req.workchain,
                    req.shard,
                    req.seqno.map(|x| x as u32),
                    req.lt,
                    req.unixtime,
                )
                .await
                .map(|r| serde_json::json!({ "ok": true, "result": api::map_block_id(&r) }))
            } else {
                Err(anyhow::anyhow!("Invalid params for lookupBlock"))
            }
        }
        _ => {
            return (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": payload.id,
                    "ok": false,
                    "error": "Method not found",
                    "code": 404
                })),
            )
                .into_response();
        }
    };

    match result {
        Ok(res) => {
            let mut response = serde_json::json!({
                "jsonrpc": "2.0",
                "id": payload.id,
            });

            if let Some(ok) = res.get("ok").and_then(|v| v.as_bool()) {
                if ok {
                    response["ok"] = serde_json::json!(true);
                    response["result"] = res.get("result").cloned().unwrap_or(res);
                    (StatusCode::OK, Json(response)).into_response()
                } else {
                    let code = res.get("code").and_then(|v| v.as_u64()).unwrap_or(500) as u16;
                    response["ok"] = serde_json::json!(false);
                    response["error"] = res
                        .get("error")
                        .cloned()
                        .unwrap_or_else(|| serde_json::json!("Unknown error"));
                    response["code"] = serde_json::json!(code);
                    (StatusCode::INTERNAL_SERVER_ERROR, Json(response)).into_response()
                }
            } else {
                response["ok"] = serde_json::json!(true);
                response["result"] = res;
                (StatusCode::OK, Json(response)).into_response()
            }
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({
                "jsonrpc": "2.0",
                "id": payload.id,
                "ok": false,
                "error": e.to_string(),
                "code": 500
            })),
        )
            .into_response(),
    }
}

pub async fn send_boc(
    State(node): State<Arc<LiteNode>>,
    Json(payload): Json<SendBocRequest>,
) -> Json<Value> {
    handle_result(node.send_boc(payload.boc), api::map_block_transactions).await
}

pub async fn run_get_method(
    State(node): State<Arc<LiteNode>>,
    Json(payload): Json<RunGetMethodRequest>,
) -> Json<Value> {
    let method_str = match payload.method {
        Value::String(s) => s,
        Value::Number(n) => n.to_string(),
        _ => {
            return Json(serde_json::json!({
                "ok": false,
                "error": "Invalid method format",
                "code": 400
            }));
        }
    };

    handle_result(
        node.run_get_method(payload.address, method_str, payload.stack, payload.seqno),
        |res| api::map_run_get_method(res, true),
    )
    .await
}

pub async fn run_get_method_std(
    State(node): State<Arc<LiteNode>>,
    Json(payload): Json<RunGetMethodRequest>,
) -> Json<Value> {
    let method_str = match payload.method {
        Value::String(s) => s,
        Value::Number(n) => n.to_string(),
        _ => {
            return Json(serde_json::json!({
                "ok": false,
                "error": "Invalid method format",
                "code": 400
            }));
        }
    };

    handle_result(
        node.run_get_method(payload.address, method_str, payload.stack, payload.seqno),
        |res| api::map_run_get_method(res, false),
    )
    .await
}

pub async fn get_address_information_query(
    State(node): State<Arc<LiteNode>>,
    Query(payload): Query<GetAddressInformationRequest>,
) -> Json<Value> {
    handle_result(
        node.get_address_information(payload.address, payload.seqno),
        api::map_account_state,
    )
    .await
}

pub async fn get_address_information_post(
    State(node): State<Arc<LiteNode>>,
    Json(payload): Json<GetAddressInformationRequest>,
) -> Json<Value> {
    handle_result(
        node.get_address_information(payload.address, payload.seqno),
        api::map_account_state,
    )
    .await
}

pub async fn get_address_balance_query(
    State(node): State<Arc<LiteNode>>,
    Query(payload): Query<GetAddressInformationRequest>,
) -> Json<Value> {
    handle_result(
        node.get_address_balance(payload.address, payload.seqno),
        |res| serde_json::json!({ "ok": true, "result": res.to_string() }),
    )
    .await
}

pub async fn get_address_balance_post(
    State(node): State<Arc<LiteNode>>,
    Json(payload): Json<GetAddressInformationRequest>,
) -> Json<Value> {
    handle_result(
        node.get_address_balance(payload.address, payload.seqno),
        |res| serde_json::json!({ "ok": true, "result": res.to_string() }),
    )
    .await
}

pub async fn get_address_state_query(
    State(node): State<Arc<LiteNode>>,
    Query(payload): Query<GetAddressInformationRequest>,
) -> Json<Value> {
    handle_result(
        node.get_address_state(payload.address, payload.seqno),
        |res| serde_json::json!({ "ok": true, "result": format!("{:?}", res).to_lowercase() }),
    )
    .await
}

pub async fn get_address_state_post(
    State(node): State<Arc<LiteNode>>,
    Json(payload): Json<GetAddressInformationRequest>,
) -> Json<Value> {
    handle_result(
        node.get_address_state(payload.address, payload.seqno),
        |res| serde_json::json!({ "ok": true, "result": format!("{:?}", res).to_lowercase() }),
    )
    .await
}

pub async fn get_extended_address_information_query(
    State(node): State<Arc<LiteNode>>,
    Query(payload): Query<GetAddressInformationRequest>,
) -> Json<Value> {
    handle_result(
        node.get_address_information(payload.address, payload.seqno),
        api::map_extended_account_state,
    )
    .await
}

pub async fn get_extended_address_information_post(
    State(node): State<Arc<LiteNode>>,
    Json(payload): Json<GetAddressInformationRequest>,
) -> Json<Value> {
    handle_result(
        node.get_address_information(payload.address, payload.seqno),
        api::map_extended_account_state,
    )
    .await
}

pub async fn get_transactions_query(
    State(node): State<Arc<LiteNode>>,
    Query(payload): Query<GetTransactionsRequest>,
) -> Json<Value> {
    handle_result(
        node.get_transactions(
            payload.address,
            payload.limit,
            payload.lt,
            payload.hash,
            payload.to_lt,
        ),
        |res| serde_json::json!({ "ok": true, "result": res.iter().map(api::map_transaction).collect::<Vec<_>>() }),
    )
    .await
}

pub async fn get_transactions_post(
    State(node): State<Arc<LiteNode>>,
    Json(payload): Json<GetTransactionsRequest>,
) -> Json<Value> {
    handle_result(
        node.get_transactions(
            payload.address,
            payload.limit,
            payload.lt,
            payload.hash,
            payload.to_lt,
        ),
        |res| serde_json::json!({ "ok": true, "result": res.iter().map(api::map_transaction).collect::<Vec<_>>() }),
    )
    .await
}

pub async fn get_block_header_query(
    State(node): State<Arc<LiteNode>>,
    Query(payload): Query<GetBlockRequest>,
) -> Json<Value> {
    handle_result(
        node.get_block_header(payload.seqno as u32),
        api::map_block_header,
    )
    .await
}

pub async fn get_block_header_post(
    State(node): State<Arc<LiteNode>>,
    Json(payload): Json<GetBlockRequest>,
) -> Json<Value> {
    handle_result(
        node.get_block_header(payload.seqno as u32),
        api::map_block_header,
    )
    .await
}

pub async fn get_block_transactions_ext_post(
    State(node): State<Arc<LiteNode>>,
    Json(payload): Json<GetBlockRequest>,
) -> Json<Value> {
    handle_result(
        node.get_block_transactions(payload.seqno as u32),
        api::map_block_transactions_ext,
    )
    .await
}

pub async fn send_boc_return_hash(
    State(node): State<Arc<LiteNode>>,
    Json(payload): Json<SendBocRequest>,
) -> Json<Value> {
    handle_result(node.send_boc(payload.boc), api::map_send_boc_return_hash).await
}

pub async fn get_block_transactions_query(
    State(node): State<Arc<LiteNode>>,
    Query(payload): Query<GetBlockRequest>,
) -> Json<Value> {
    handle_result(
        node.get_block_transactions(payload.seqno as u32),
        api::map_block_transactions,
    )
    .await
}

pub async fn get_block_transactions_post(
    State(node): State<Arc<LiteNode>>,
    Json(payload): Json<GetBlockRequest>,
) -> Json<Value> {
    handle_result(
        node.get_block_transactions(payload.seqno as u32),
        api::map_block_transactions,
    )
    .await
}

pub async fn get_block_transactions_ext_query(
    State(node): State<Arc<LiteNode>>,
    Query(payload): Query<GetBlockRequest>,
) -> Json<Value> {
    handle_result(
        node.get_block_transactions(payload.seqno as u32),
        api::map_block_transactions_ext,
    )
    .await
}

pub async fn get_masterchain_info_query(State(node): State<Arc<LiteNode>>) -> Json<Value> {
    handle_result(node.get_masterchain_info(), api::map_masterchain_info).await
}

pub async fn get_masterchain_info_post(State(node): State<Arc<LiteNode>>) -> Json<Value> {
    handle_result(node.get_masterchain_info(), api::map_masterchain_info).await
}

pub async fn get_shards_query(
    State(node): State<Arc<LiteNode>>,
    Query(payload): Query<GetBlockRequest>,
) -> Json<Value> {
    handle_result(node.get_shards(payload.seqno as u32), |res| {
        serde_json::json!({
            "ok": true,
            "result": {
                "@type": "blocks.shards",
                "shards": res.iter().map(api::map_block_id).collect::<Vec<_>>()
            }
        })
    })
    .await
}

pub async fn get_shards_post(
    State(node): State<Arc<LiteNode>>,
    Json(payload): Json<GetBlockRequest>,
) -> Json<Value> {
    handle_result(node.get_shards(payload.seqno as u32), |res| {
        serde_json::json!({
            "ok": true,
            "result": {
                "@type": "blocks.shards",
                "shards": res.iter().map(api::map_block_id).collect::<Vec<_>>()
            }
        })
    })
    .await
}

pub async fn lookup_block_query(
    State(node): State<Arc<LiteNode>>,
    Query(payload): Query<LookupBlockRequest>,
) -> Json<Value> {
    handle_result(
        node.lookup_block(
            payload.workchain,
            payload.shard,
            payload.seqno.map(|x| x as u32),
            payload.lt,
            payload.unixtime,
        ),
        |res| serde_json::json!({ "ok": true, "result": api::map_block_id(res) }),
    )
    .await
}

pub async fn lookup_block_post(
    State(node): State<Arc<LiteNode>>,
    Json(payload): Json<LookupBlockRequest>,
) -> Json<Value> {
    handle_result(
        node.lookup_block(
            payload.workchain,
            payload.shard,
            payload.seqno.map(|x| x as u32),
            payload.lt,
            payload.unixtime,
        ),
        |res| serde_json::json!({ "ok": true, "result": api::map_block_id(res) }),
    )
    .await
}

pub async fn faucet(
    State(node): State<Arc<LiteNode>>,
    Json(payload): Json<FaucetRequest>,
) -> Json<Value> {
    handle_result(node.faucet(payload.address, payload.amount), |res| {
        res.clone()
    })
    .await
}

pub async fn get_traces_query(
    State(node): State<Arc<LiteNode>>,
    Query(payload): Query<GetTracesQuery>,
) -> Json<Value> {
    handle_result(node.get_traces(payload.hash), toncenter_v3::map_traces).await
}

pub async fn get_state_source(State(node): State<Arc<LiteNode>>) -> Json<Value> {
    handle_result(
        node.get_state_source(),
        |res| serde_json::json!({ "ok": true, "result": res }),
    )
    .await
}

pub async fn set_state_source(
    State(node): State<Arc<LiteNode>>,
    Json(payload): Json<node::StateSource>,
) -> Json<Value> {
    handle_result(
        node.set_state_source(payload),
        |_| serde_json::json!({ "ok": true }),
    )
    .await
}

async fn handle_result<T, F>(
    result: impl Future<Output = anyhow::Result<T>>,
    mapper: F,
) -> Json<Value>
where
    F: FnOnce(&T) -> Value,
{
    match result.await {
        Ok(res) => Json(mapper(&res)),
        Err(e) => Json(serde_json::json!({
            "ok": false,
            "error": e.to_string(),
            "code": 500
        })),
    }
}
