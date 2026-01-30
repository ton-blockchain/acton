use crate::litenode::LiteNode;
use axum::{Json, Router, extract::State, routing::post};
use serde::Deserialize;
use serde_json::Value;
use std::sync::Arc;

pub(crate) async fn run_server(node: Arc<LiteNode>, port: u16) -> anyhow::Result<()> {
    let app = Router::new()
        .route("/api/v2/sendBoc", post(send_boc))
        .route("/api/v2/runGetMethod", post(run_get_method))
        .with_state(node);

    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{}", port)).await?;
    tracing::info!("Server running on http://0.0.0.0:{}", port);
    axum::serve(listener, app).await?;
    Ok(())
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
            "@type": "error",
            "code": 500,
            "message": e.to_string()
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
            "@type": "error",
            "code": 500,
            "message": e.to_string()
        })),
    }
}
