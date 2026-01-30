use crate::litenode::LiteNode;
use axum::{
    Json, Router,
    extract::{Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
};
use serde::Deserialize;
use serde_json::Value;
use std::sync::Arc;
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;

pub async fn run_server(node: Arc<LiteNode>, port: u16) -> anyhow::Result<()> {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let app = Router::new()
        .route("/api/v2", post(json_rpc))
        .route("/api/v2/jsonRPC", post(json_rpc))
        .route("/api/v2/v2/jsonRPC", post(json_rpc))
        .route("/api/v2/sendBoc", post(send_boc))
        .route("/api/v2/runGetMethod", post(run_get_method))
        .route("/api/v2/runGetMethodStd", post(run_get_method_std))
        .route(
            "/api/v2/getAddressInformation",
            get(get_address_information_query).post(get_address_information_post),
        )
        .route(
            "/api/v2/getAddressBalance",
            get(get_address_balance_query).post(get_address_balance_post),
        )
        .route(
            "/api/v2/getAddressState",
            get(get_address_state_query).post(get_address_state_post),
        )
        .route(
            "/api/v2/getExtendedAddressInformation",
            get(get_extended_address_information_query).post(get_extended_address_information_post),
        )
        .route(
            "/api/v2/getTransactions",
            get(get_transactions_query).post(get_transactions_post),
        )
        .route(
            "/api/v2/getBlockHeader",
            get(get_block_header_query).post(get_block_header_post),
        )
        .route(
            "/api/v2/getBlockTransactionsExt",
            get(get_block_transactions_ext_query).post(get_block_transactions_ext_post),
        )
        .route(
            "/api/v2/getMasterchainInfo",
            get(get_masterchain_info_query).post(get_masterchain_info_post),
        )
        .route("/faucet", post(faucet))
        .layer(cors)
        .layer(TraceLayer::new_for_http())
        .with_state(node);

    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{}", port)).await?;
    tracing::info!("Server running on http://0.0.0.0:{}", port);
    axum::serve(listener, app).await?;
    Ok(())
}

#[derive(Deserialize)]
struct JsonRpcRequest {
    #[serde(rename = "jsonrpc")]
    _jsonrpc: String,
    id: Value,
    method: String,
    params: Value,
}

async fn json_rpc(
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
                node.send_boc(req.boc).await
            } else {
                Err(anyhow::anyhow!("Invalid params for sendBoc"))
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
                node.run_get_method_std(req.address, method_str, req.stack, req.seqno)
                    .await
            } else {
                Err(anyhow::anyhow!("Invalid params for runGetMethodStd"))
            }
        }
        "getAddressInformation" => {
            if let Ok(req) = serde_json::from_value::<GetAddressInformationRequest>(payload.params)
            {
                node.get_address_information(req.address, req.seqno).await
            } else {
                Err(anyhow::anyhow!("Invalid params for getAddressInformation"))
            }
        }
        "getAddressBalance" => {
            if let Ok(req) = serde_json::from_value::<GetAddressInformationRequest>(payload.params)
            {
                node.get_address_balance(req.address, req.seqno).await
            } else {
                Err(anyhow::anyhow!("Invalid params for getAddressBalance"))
            }
        }
        "getAddressState" => {
            if let Ok(req) = serde_json::from_value::<GetAddressInformationRequest>(payload.params)
            {
                node.get_address_state(req.address, req.seqno).await
            } else {
                Err(anyhow::anyhow!("Invalid params for getAddressState"))
            }
        }
        "getExtendedAddressInformation" => {
            if let Ok(req) = serde_json::from_value::<GetAddressInformationRequest>(payload.params)
            {
                node.get_extended_address_information(req.address, req.seqno)
                    .await
            } else {
                Err(anyhow::anyhow!(
                    "Invalid params for getExtendedAddressInformation"
                ))
            }
        }
        "getTransactions" => {
            if let Ok(req) = serde_json::from_value::<GetTransactionsRequest>(payload.params) {
                node.get_transactions(req.address, req.limit, req.lt, req.hash, req.to_lt)
                    .await
            } else {
                Err(anyhow::anyhow!("Invalid params for getTransactions"))
            }
        }
        "getBlockHeader" => {
            if let Ok(req) = serde_json::from_value::<GetBlockRequest>(payload.params) {
                node.get_block_header(req.seqno).await
            } else {
                Err(anyhow::anyhow!("Invalid params for getBlockHeader"))
            }
        }
        "getBlockTransactionsExt" => {
            if let Ok(req) = serde_json::from_value::<GetBlockRequest>(payload.params) {
                node.get_block_transactions_ext(req.seqno).await
            } else {
                Err(anyhow::anyhow!(
                    "Invalid params for getBlockTransactionsExt"
                ))
            }
        }
        "getMasterchainInfo" => node.get_masterchain_info().await,
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

#[derive(Deserialize)]
struct SendBocRequest {
    boc: String,
}

async fn send_boc(
    State(node): State<Arc<LiteNode>>,
    Json(payload): Json<SendBocRequest>,
) -> Json<Value> {
    match node.send_boc(payload.boc).await {
        Ok(res) => Json(res),
        Err(e) => Json(serde_json::json!({
            "ok": false,
            "error": e.to_string(),
            "code": 500
        })),
    }
}

#[derive(Deserialize)]
struct RunGetMethodRequest {
    address: String,
    method: Value, // String or Integer
    stack: Vec<Value>,
    seqno: Option<u32>,
}

async fn run_get_method(
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

    match node
        .run_get_method(payload.address, method_str, payload.stack, payload.seqno)
        .await
    {
        Ok(res) => Json(res),
        Err(e) => Json(serde_json::json!({
            "ok": false,
            "error": e.to_string(),
            "code": 500
        })),
    }
}

async fn run_get_method_std(
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

    match node
        .run_get_method_std(payload.address, method_str, payload.stack, payload.seqno)
        .await
    {
        Ok(res) => Json(res),
        Err(e) => Json(serde_json::json!({
            "ok": false,
            "error": e.to_string(),
            "code": 500
        })),
    }
}

#[derive(Deserialize)]
struct GetAddressInformationRequest {
    address: String,
    seqno: Option<u32>,
}

async fn get_address_information_query(
    State(node): State<Arc<LiteNode>>,
    Query(payload): Query<GetAddressInformationRequest>,
) -> Json<Value> {
    match node
        .get_address_information(payload.address, payload.seqno)
        .await
    {
        Ok(res) => Json(res),
        Err(e) => Json(serde_json::json!({
            "ok": false,
            "error": e.to_string(),
            "code": 500
        })),
    }
}

async fn get_address_information_post(
    State(node): State<Arc<LiteNode>>,
    Json(payload): Json<GetAddressInformationRequest>,
) -> Json<Value> {
    match node
        .get_address_information(payload.address, payload.seqno)
        .await
    {
        Ok(res) => Json(res),
        Err(e) => Json(serde_json::json!({
            "ok": false,
            "error": e.to_string(),
            "code": 500
        })),
    }
}

async fn get_address_balance_query(
    State(node): State<Arc<LiteNode>>,
    Query(payload): Query<GetAddressInformationRequest>,
) -> Json<Value> {
    match node
        .get_address_balance(payload.address, payload.seqno)
        .await
    {
        Ok(res) => Json(res),
        Err(e) => Json(serde_json::json!({
            "ok": false,
            "error": e.to_string(),
            "code": 500
        })),
    }
}

async fn get_address_balance_post(
    State(node): State<Arc<LiteNode>>,
    Json(payload): Json<GetAddressInformationRequest>,
) -> Json<Value> {
    match node
        .get_address_balance(payload.address, payload.seqno)
        .await
    {
        Ok(res) => Json(res),
        Err(e) => Json(serde_json::json!({
            "ok": false,
            "error": e.to_string(),
            "code": 500
        })),
    }
}

async fn get_address_state_query(
    State(node): State<Arc<LiteNode>>,
    Query(payload): Query<GetAddressInformationRequest>,
) -> Json<Value> {
    match node.get_address_state(payload.address, payload.seqno).await {
        Ok(res) => Json(res),
        Err(e) => Json(serde_json::json!({
            "ok": false,
            "error": e.to_string(),
            "code": 500
        })),
    }
}

async fn get_address_state_post(
    State(node): State<Arc<LiteNode>>,
    Json(payload): Json<GetAddressInformationRequest>,
) -> Json<Value> {
    match node.get_address_state(payload.address, payload.seqno).await {
        Ok(res) => Json(res),
        Err(e) => Json(serde_json::json!({
            "ok": false,
            "error": e.to_string(),
            "code": 500
        })),
    }
}

async fn get_extended_address_information_query(
    State(node): State<Arc<LiteNode>>,
    Query(payload): Query<GetAddressInformationRequest>,
) -> Json<Value> {
    match node
        .get_extended_address_information(payload.address, payload.seqno)
        .await
    {
        Ok(res) => Json(res),
        Err(e) => Json(serde_json::json!({
            "ok": false,
            "error": e.to_string(),
            "code": 500
        })),
    }
}

async fn get_extended_address_information_post(
    State(node): State<Arc<LiteNode>>,
    Json(payload): Json<GetAddressInformationRequest>,
) -> Json<Value> {
    match node
        .get_extended_address_information(payload.address, payload.seqno)
        .await
    {
        Ok(res) => Json(res),
        Err(e) => Json(serde_json::json!({
            "ok": false,
            "error": e.to_string(),
            "code": 500
        })),
    }
}

#[derive(Deserialize)]
struct GetTransactionsRequest {
    address: String,
    #[serde(default = "default_limit")]
    limit: u32,
    lt: Option<u64>,
    hash: Option<String>,
    to_lt: Option<u64>,
}

const fn default_limit() -> u32 {
    10
}

async fn get_transactions_query(
    State(node): State<Arc<LiteNode>>,
    Query(payload): Query<GetTransactionsRequest>,
) -> Json<Value> {
    match node
        .get_transactions(
            payload.address,
            payload.limit,
            payload.lt,
            payload.hash,
            payload.to_lt,
        )
        .await
    {
        Ok(res) => Json(res),
        Err(e) => Json(serde_json::json!({
            "ok": false,
            "error": e.to_string(),
            "code": 500
        })),
    }
}

async fn get_transactions_post(
    State(node): State<Arc<LiteNode>>,
    Json(payload): Json<GetTransactionsRequest>,
) -> Json<Value> {
    match node
        .get_transactions(
            payload.address,
            payload.limit,
            payload.lt,
            payload.hash,
            payload.to_lt,
        )
        .await
    {
        Ok(res) => Json(res),
        Err(e) => Json(serde_json::json!({
            "ok": false,
            "error": e.to_string(),
            "code": 500
        })),
    }
}

#[derive(Deserialize)]
struct GetBlockRequest {
    /// Workchain index (ignored, dev node only uses workchain 0)
    #[allow(dead_code)]
    workchain: Option<i32>,
    /// Shard ID (ignored, dev node only uses shard -9223372036854775808)
    #[allow(dead_code)]
    shard: Option<i64>,
    seqno: u32,
}

async fn get_block_header_query(
    State(node): State<Arc<LiteNode>>,
    Query(payload): Query<GetBlockRequest>,
) -> Json<Value> {
    match node.get_block_header(payload.seqno).await {
        Ok(res) => Json(res),
        Err(e) => Json(serde_json::json!({
            "ok": false,
            "error": e.to_string(),
            "code": 500
        })),
    }
}

async fn get_block_header_post(
    State(node): State<Arc<LiteNode>>,
    Json(payload): Json<GetBlockRequest>,
) -> Json<Value> {
    match node.get_block_header(payload.seqno).await {
        Ok(res) => Json(res),
        Err(e) => Json(serde_json::json!({
            "ok": false,
            "error": e.to_string(),
            "code": 500
        })),
    }
}

async fn get_block_transactions_ext_query(
    State(node): State<Arc<LiteNode>>,
    Query(payload): Query<GetBlockRequest>,
) -> Json<Value> {
    match node.get_block_transactions_ext(payload.seqno).await {
        Ok(res) => Json(res),
        Err(e) => Json(serde_json::json!({
            "ok": false,
            "error": e.to_string(),
            "code": 500
        })),
    }
}

async fn get_block_transactions_ext_post(
    State(node): State<Arc<LiteNode>>,
    Json(payload): Json<GetBlockRequest>,
) -> Json<Value> {
    match node.get_block_transactions_ext(payload.seqno).await {
        Ok(res) => Json(res),
        Err(e) => Json(serde_json::json!({
            "ok": false,
            "error": e.to_string(),
            "code": 500
        })),
    }
}

async fn get_masterchain_info_query(State(node): State<Arc<LiteNode>>) -> Json<Value> {
    match node.get_masterchain_info().await {
        Ok(res) => Json(res),
        Err(e) => Json(serde_json::json!({
            "ok": false,
            "error": e.to_string(),
            "code": 500
        })),
    }
}

async fn get_masterchain_info_post(State(node): State<Arc<LiteNode>>) -> Json<Value> {
    match node.get_masterchain_info().await {
        Ok(res) => Json(res),
        Err(e) => Json(serde_json::json!({
            "ok": false,
            "error": e.to_string(),
            "code": 500
        })),
    }
}

#[derive(Deserialize)]
struct FaucetRequest {
    address: String,
    amount: u128,
}

async fn faucet(
    State(node): State<Arc<LiteNode>>,
    Json(payload): Json<FaucetRequest>,
) -> Json<Value> {
    match node.faucet(payload.address, payload.amount).await {
        Ok(res) => Json(res),
        Err(e) => Json(serde_json::json!({
            "ok": false,
            "error": e.to_string(),
            "code": 500
        })),
    }
}
