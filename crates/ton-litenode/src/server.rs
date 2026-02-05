use crate::api::toncenter_v3;
use crate::litenode::LiteNode;
use crate::{api, node};
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

#[derive(Debug)]
pub struct ServerArgs {
    pub port: u16,
    pub db_path: Option<String>,
}

pub async fn run_server(node: Arc<LiteNode>, args: ServerArgs) -> anyhow::Result<()> {
    let port = args.port;

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let api_router = Router::new()
        .route("/v2", post(json_rpc))
        .route("/v2/jsonRPC", post(json_rpc))
        .route("/v2/v2/jsonRPC", post(json_rpc))
        .route("/v2/sendBoc", post(send_boc))
        .route("/v2/sendBocReturnHash", post(send_boc_return_hash))
        .route("/v2/runGetMethod", post(run_get_method))
        .route("/v2/runGetMethodStd", post(run_get_method_std))
        .route(
            "/v2/getAddressInformation",
            get(get_address_information_query).post(get_address_information_post),
        )
        .route(
            "/v2/getAddressBalance",
            get(get_address_balance_query).post(get_address_balance_post),
        )
        .route(
            "/v2/getAddressState",
            get(get_address_state_query).post(get_address_state_post),
        )
        .route(
            "/v2/getExtendedAddressInformation",
            get(get_extended_address_information_query).post(get_extended_address_information_post),
        )
        .route(
            "/v2/getTransactions",
            get(get_transactions_query).post(get_transactions_post),
        )
        .route(
            "/v2/getBlockHeader",
            get(get_block_header_query).post(get_block_header_post),
        )
        .route(
            "/v2/getBlockTransactions",
            get(get_block_transactions_query).post(get_block_transactions_post),
        )
        .route(
            "/v2/getBlockTransactionsExt",
            get(get_block_transactions_ext_query).post(get_block_transactions_ext_post),
        )
        .route(
            "/v2/getMasterchainInfo",
            get(get_masterchain_info_query).post(get_masterchain_info_post),
        )
        .route("/v2/getShards", get(get_shards_query).post(get_shards_post))
        .route("/v2/shards", get(get_shards_query).post(get_shards_post))
        .route(
            "/v2/lookupBlock",
            get(lookup_block_query).post(lookup_block_post),
        )
        .route("/v3/traces", get(get_traces_query));

    let app = Router::new()
        .nest("/api", api_router.clone())
        .route("/faucet", post(faucet))
        .route(
            "/state-source",
            get(get_state_source).post(set_state_source),
        )
        .layer(cors)
        .layer(TraceLayer::new_for_http())
        .with_state(node.clone());

    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{}", port)).await?;
    println!(
        "     \x1b[1;32mStarting\x1b[0m LiteNode server on http://0.0.0.0:{}",
        port
    );
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

#[derive(Deserialize)]
struct SendBocRequest {
    boc: String,
}

async fn send_boc(
    State(node): State<Arc<LiteNode>>,
    Json(payload): Json<SendBocRequest>,
) -> Json<Value> {
    match node.send_boc(payload.boc).await {
        Ok(res) => Json(api::map_block_transactions(&res)),
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
        Ok(res) => Json(api::map_run_get_method(&res, true)),
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
        .run_get_method(payload.address, method_str, payload.stack, payload.seqno)
        .await
    {
        Ok(res) => Json(api::map_run_get_method(&res, false)),
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
        Ok(res) => Json(api::map_account_state(&res)),
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
        Ok(res) => Json(api::map_account_state(&res)),
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
        Ok(res) => Json(serde_json::json!({ "ok": true, "result": res.to_string() })),
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
        Ok(res) => Json(serde_json::json!({ "ok": true, "result": res.to_string() })),
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
        Ok(res) => {
            Json(serde_json::json!({ "ok": true, "result": format!("{:?}", res).to_lowercase() }))
        }
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
        Ok(res) => {
            Json(serde_json::json!({ "ok": true, "result": format!("{:?}", res).to_lowercase() }))
        }
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
        .get_address_information(payload.address, payload.seqno)
        .await
    {
        Ok(res) => Json(api::map_extended_account_state(&res)),
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
        .get_address_information(payload.address, payload.seqno)
        .await
    {
        Ok(res) => Json(api::map_extended_account_state(&res)),
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
    limit: usize,
    lt: Option<u64>,
    hash: Option<String>,
    to_lt: Option<u64>,
}

const fn default_limit() -> usize {
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
        Ok(res) => Json(
            serde_json::json!({ "ok": true, "result": res.iter().map(api::map_transaction).collect::<Vec<_>>() }),
        ),
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
        Ok(res) => Json(
            serde_json::json!({ "ok": true, "result": res.iter().map(api::map_transaction).collect::<Vec<_>>() }),
        ),
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
    shard: Option<String>,
    seqno: i32,
}

async fn get_block_header_query(
    State(node): State<Arc<LiteNode>>,
    Query(payload): Query<GetBlockRequest>,
) -> Json<Value> {
    match node.get_block_header(payload.seqno as u32).await {
        Ok(res) => Json(api::map_block_header(&res)),
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
    match node.get_block_header(payload.seqno as u32).await {
        Ok(res) => Json(api::map_block_header(&res)),
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
    match node.get_block_transactions(payload.seqno as u32).await {
        Ok(res) => Json(api::map_block_transactions_ext(&res)),
        Err(e) => Json(serde_json::json!({
            "ok": false,
            "error": e.to_string(),
            "code": 500
        })),
    }
}

async fn send_boc_return_hash(
    State(node): State<Arc<LiteNode>>,
    Json(payload): Json<SendBocRequest>,
) -> Json<Value> {
    match node.send_boc(payload.boc).await {
        Ok(res) => Json(api::map_send_boc_return_hash(&res)),
        Err(e) => Json(serde_json::json!({
            "ok": false,
            "error": e.to_string(),
            "code": 500
        })),
    }
}

async fn get_block_transactions_query(
    State(node): State<Arc<LiteNode>>,
    Query(payload): Query<GetBlockRequest>,
) -> Json<Value> {
    match node.get_block_transactions(payload.seqno as u32).await {
        Ok(res) => Json(api::map_block_transactions(&res)),
        Err(e) => Json(serde_json::json!({
            "ok": false,
            "error": e.to_string(),
            "code": 500
        })),
    }
}

async fn get_block_transactions_post(
    State(node): State<Arc<LiteNode>>,
    Json(payload): Json<GetBlockRequest>,
) -> Json<Value> {
    match node.get_block_transactions(payload.seqno as u32).await {
        Ok(res) => Json(api::map_block_transactions(&res)),
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
    match node.get_block_transactions(payload.seqno as u32).await {
        Ok(res) => Json(api::map_block_transactions_ext(&res)),
        Err(e) => Json(serde_json::json!({
            "ok": false,
            "error": e.to_string(),
            "code": 500
        })),
    }
}

async fn get_masterchain_info_query(State(node): State<Arc<LiteNode>>) -> Json<Value> {
    match node.get_masterchain_info().await {
        Ok(res) => Json(api::map_masterchain_info(&res)),
        Err(e) => Json(serde_json::json!({
            "ok": false,
            "error": e.to_string(),
            "code": 500
        })),
    }
}

async fn get_masterchain_info_post(State(node): State<Arc<LiteNode>>) -> Json<Value> {
    match node.get_masterchain_info().await {
        Ok(res) => Json(api::map_masterchain_info(&res)),
        Err(e) => Json(serde_json::json!({
            "ok": false,
            "error": e.to_string(),
            "code": 500
        })),
    }
}

async fn get_shards_query(
    State(node): State<Arc<LiteNode>>,
    Query(payload): Query<GetBlockRequest>,
) -> Json<Value> {
    match node.get_shards(payload.seqno as u32).await {
        Ok(res) => Json(serde_json::json!({
            "ok": true,
            "result": {
                "@type": "blocks.shards",
                "shards": res.iter().map(api::map_block_id).collect::<Vec<_>>()
            }
        })),
        Err(e) => Json(serde_json::json!({
            "ok": false,
            "error": e.to_string(),
            "code": 500
        })),
    }
}

async fn get_shards_post(
    State(node): State<Arc<LiteNode>>,
    Json(payload): Json<GetBlockRequest>,
) -> Json<Value> {
    match node.get_shards(payload.seqno as u32).await {
        Ok(res) => Json(serde_json::json!({
            "ok": true,
            "result": {
                "@type": "blocks.shards",
                "shards": res.iter().map(api::map_block_id).collect::<Vec<_>>()
            }
        })),
        Err(e) => Json(serde_json::json!({
            "ok": false,
            "error": e.to_string(),
            "code": 500
        })),
    }
}

#[derive(Deserialize)]
struct LookupBlockRequest {
    workchain: i32,
    shard: String,
    seqno: Option<i32>,
    lt: Option<u64>,
    unixtime: Option<u32>,
}

async fn lookup_block_query(
    State(node): State<Arc<LiteNode>>,
    Query(payload): Query<LookupBlockRequest>,
) -> Json<Value> {
    match node
        .lookup_block(
            payload.workchain,
            payload.shard,
            payload.seqno.map(|x| x as u32),
            payload.lt,
            payload.unixtime,
        )
        .await
    {
        Ok(res) => Json(serde_json::json!({ "ok": true, "result": api::map_block_id(&res) })),
        Err(e) => Json(serde_json::json!({
            "ok": false,
            "error": e.to_string(),
            "code": 500
        })),
    }
}

async fn lookup_block_post(
    State(node): State<Arc<LiteNode>>,
    Json(payload): Json<LookupBlockRequest>,
) -> Json<Value> {
    match node
        .lookup_block(
            payload.workchain,
            payload.shard,
            payload.seqno.map(|x| x as u32),
            payload.lt,
            payload.unixtime,
        )
        .await
    {
        Ok(res) => Json(serde_json::json!({ "ok": true, "result": api::map_block_id(&res) })),
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

#[derive(Deserialize)]
struct GetTracesQuery {
    hash: String,
}

async fn get_traces_query(
    State(node): State<Arc<LiteNode>>,
    Query(payload): Query<GetTracesQuery>,
) -> Json<Value> {
    match node.get_traces(payload.hash).await {
        Ok(res) => Json(toncenter_v3::map_traces(&res)),
        Err(e) => Json(serde_json::json!({
            "ok": false,
            "error": e.to_string(),
            "code": 500
        })),
    }
}

async fn get_state_source(State(node): State<Arc<LiteNode>>) -> Json<Value> {
    match node.get_state_source().await {
        Ok(res) => Json(serde_json::json!({ "ok": true, "result": res })),
        Err(e) => Json(serde_json::json!({ "ok": false, "error": e.to_string() })),
    }
}

async fn set_state_source(
    State(node): State<Arc<LiteNode>>,
    Json(payload): Json<node::StateSource>,
) -> Json<Value> {
    match node.set_state_source(payload).await {
        Ok(_) => Json(serde_json::json!({ "ok": true })),
        Err(e) => Json(serde_json::json!({ "ok": false, "error": e.to_string() })),
    }
}
