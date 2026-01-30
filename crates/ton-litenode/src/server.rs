use crate::litenode::LiteNode;
use axum::{
    extract::{Query, State},
    routing::{get, post},
    Json, Router,
};
use serde::Deserialize;
use serde_json::Value;
use std::sync::Arc;
use tower_http::cors::{Any, CorsLayer};

pub(crate) async fn run_server(node: Arc<LiteNode>, port: u16) -> anyhow::Result<()> {
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
        .layer(cors)
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
) -> Json<Value> {
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
                    _ => return Json(serde_json::json!({
                        "jsonrpc": "2.0",
                        "id": payload.id,
                        "error": { "code": -32602, "message": "Invalid method format" }
                    })),
                };
                node.run_get_method(req.address, method_str, req.stack).await
            } else {
                Err(anyhow::anyhow!("Invalid params for runGetMethod"))
            }
        }
        "getAddressInformation" => {
            if let Ok(req) = serde_json::from_value::<GetAddressInformationRequest>(payload.params)
            {
                node.get_address_information(req.address).await
            } else {
                Err(anyhow::anyhow!("Invalid params for getAddressInformation"))
            }
        }
        "getAddressBalance" => {
            if let Ok(req) = serde_json::from_value::<GetAddressInformationRequest>(payload.params)
            {
                node.get_address_balance(req.address).await
            } else {
                Err(anyhow::anyhow!("Invalid params for getAddressBalance"))
            }
        }
        "getAddressState" => {
            if let Ok(req) = serde_json::from_value::<GetAddressInformationRequest>(payload.params)
            {
                node.get_address_state(req.address).await
            } else {
                Err(anyhow::anyhow!("Invalid params for getAddressState"))
            }
        }
        "getExtendedAddressInformation" => {
            if let Ok(req) = serde_json::from_value::<GetAddressInformationRequest>(payload.params)
            {
                node.get_extended_address_information(req.address).await
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
        _ => {
            return Json(serde_json::json!({
                "jsonrpc": "2.0",
                "id": payload.id,
                "error": { "code": -32601, "message": "Method not found" }
            }));
        }
    };

    match result {
        Ok(res) => Json(serde_json::json!({
            "jsonrpc": "2.0",
            "id": payload.id,
            "result": res.get("result").unwrap_or(&res)
        })),
        Err(e) => Json(serde_json::json!({
            "jsonrpc": "2.0",
            "id": payload.id,
            "error": { "code": -32603, "message": e.to_string() }
        })),
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
            "code": 500,
            "error": e.to_string()
        })),
    }
}

#[derive(Deserialize)]
struct RunGetMethodRequest {
    address: String,
    method: Value, // String or Integer
    stack: Vec<Value>,
}

async fn run_get_method(
    State(node): State<Arc<LiteNode>>,
    Json(payload): Json<RunGetMethodRequest>,
) -> Json<Value> {
    let method_str = match payload.method {
        Value::String(s) => s,
        Value::Number(n) => n.to_string(),
        _ => {
            return Json(
                serde_json::json!({ "@type": "error", "message": "Invalid method format" }),
            );
        }
    };

    match node
        .run_get_method(payload.address, method_str, payload.stack)
        .await
    {
        Ok(res) => Json(res),
        Err(e) => Json(serde_json::json!({
            "ok": false,
            "code": 500,
            "error": e.to_string()
        })),
    }
}

#[derive(Deserialize)]
struct GetAddressInformationRequest {
    address: String,
}

async fn get_address_information_query(
    State(node): State<Arc<LiteNode>>,
    Query(payload): Query<GetAddressInformationRequest>,
) -> Json<Value> {
    match node.get_address_information(payload.address).await {
        Ok(res) => Json(res),
        Err(e) => Json(serde_json::json!({
            "ok": false,
            "code": 500,
            "error": e.to_string()
        })),
    }
}

async fn get_address_information_post(
    State(node): State<Arc<LiteNode>>,
    Json(payload): Json<GetAddressInformationRequest>,
) -> Json<Value> {
    match node.get_address_information(payload.address).await {
        Ok(res) => Json(res),
        Err(e) => Json(serde_json::json!({
            "ok": false,
            "code": 500,
            "error": e.to_string()
        })),
    }
}

async fn get_address_balance_query(
    State(node): State<Arc<LiteNode>>,
    Query(payload): Query<GetAddressInformationRequest>,
) -> Json<Value> {
    match node.get_address_balance(payload.address).await {
        Ok(res) => Json(res),
        Err(e) => Json(serde_json::json!({
            "ok": false,
            "code": 500,
            "error": e.to_string()
        })),
    }
}

async fn get_address_balance_post(
    State(node): State<Arc<LiteNode>>,
    Json(payload): Json<GetAddressInformationRequest>,
) -> Json<Value> {
    match node.get_address_balance(payload.address).await {
        Ok(res) => Json(res),
        Err(e) => Json(serde_json::json!({
            "ok": false,
            "code": 500,
            "error": e.to_string()
        })),
    }
}

async fn get_address_state_query(
    State(node): State<Arc<LiteNode>>,
    Query(payload): Query<GetAddressInformationRequest>,
) -> Json<Value> {
    match node.get_address_state(payload.address).await {
        Ok(res) => Json(res),
        Err(e) => Json(serde_json::json!({
            "ok": false,
            "code": 500,
            "error": e.to_string()
        })),
    }
}

async fn get_address_state_post(
    State(node): State<Arc<LiteNode>>,
    Json(payload): Json<GetAddressInformationRequest>,
) -> Json<Value> {
    match node.get_address_state(payload.address).await {
        Ok(res) => Json(res),
        Err(e) => Json(serde_json::json!({
            "ok": false,
            "code": 500,
            "error": e.to_string()
        })),
    }
}

async fn get_extended_address_information_query(
    State(node): State<Arc<LiteNode>>,
    Query(payload): Query<GetAddressInformationRequest>,
) -> Json<Value> {
    match node.get_extended_address_information(payload.address).await {
        Ok(res) => Json(res),
        Err(e) => Json(serde_json::json!({
            "ok": false,
            "code": 500,
            "error": e.to_string()
        })),
    }
}

async fn get_extended_address_information_post(
    State(node): State<Arc<LiteNode>>,
    Json(payload): Json<GetAddressInformationRequest>,
) -> Json<Value> {
    match node.get_extended_address_information(payload.address).await {
        Ok(res) => Json(res),
        Err(e) => Json(serde_json::json!({
            "ok": false,
            "code": 500,
            "error": e.to_string()
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
            "code": 500,
            "error": e.to_string()
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
            "code": 500,
            "error": e.to_string()
        })),
    }
}
