use super::utils::handle_result;
use crate::litenode::LiteNode;
use crate::node;
use crate::server::models::*;
use axum::{Json, extract::State};
use serde_json::Value;
use std::sync::Arc;

pub async fn faucet(
    State(node): State<Arc<LiteNode>>,
    Json(payload): Json<FaucetRequest>,
) -> Json<Value> {
    handle_result(node.faucet(payload.address, payload.amount), |res| {
        res.clone()
    })
    .await
}

pub async fn get_state_source(State(node): State<Arc<LiteNode>>) -> Json<Value> {
    handle_result(node.get_state_source(), |res| {
        serde_json::to_value(res).unwrap_or(Value::Null)
    })
    .await
}

pub async fn set_state_source(
    State(node): State<Arc<LiteNode>>,
    Json(payload): Json<node::StateSource>,
) -> Json<Value> {
    handle_result(node.set_state_source(payload), |_| Value::Null).await
}
