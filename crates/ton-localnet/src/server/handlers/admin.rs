use super::utils::handle_result;
use crate::localnet::Localnet;
use crate::node;
use crate::server::models::*;
use crate::types::Hash256;
use axum::{Json, extract::State};
use serde_json::Value;
use std::sync::Arc;

pub async fn faucet(
    State(node): State<Arc<Localnet>>,
    Json(payload): Json<FaucetRequest>,
) -> Json<Value> {
    handle_result(node.faucet(payload.address, payload.amount), |res| {
        res.clone()
    })
    .await
}

pub async fn get_state_source(State(node): State<Arc<Localnet>>) -> Json<Value> {
    handle_result(node.get_state_source(), |res| {
        serde_json::to_value(res).unwrap_or(Value::Null)
    })
    .await
}

pub async fn set_state_source(
    State(node): State<Arc<Localnet>>,
    Json(payload): Json<node::StateSource>,
) -> Json<Value> {
    handle_result(node.set_state_source(payload), |()| Value::Null).await
}

pub async fn set_address_name(
    State(node): State<Arc<Localnet>>,
    Json(payload): Json<SetAddressNameRequest>,
) -> Json<Value> {
    handle_result(node.set_address_name(payload.address, payload.name), |()| {
        Value::Null
    })
    .await
}

pub async fn get_address_name(
    State(node): State<Arc<Localnet>>,
    axum::extract::Query(payload): axum::extract::Query<GetAddressNameQuery>,
) -> Json<Value> {
    handle_result(node.get_address_name(payload.address), |res| {
        serde_json::to_value(res).unwrap_or(Value::Null)
    })
    .await
}

pub async fn register_compiler_abis(
    State(node): State<Arc<Localnet>>,
    Json(payload): Json<RegisterCompilerAbisRequest>,
) -> Json<Value> {
    handle_result(
        async move {
            let entries = payload
                .entries
                .into_iter()
                .map(|entry| Ok((parse_hash_any(&entry.code_hash)?, entry.compiler_abi)))
                .collect::<anyhow::Result<Vec<_>>>()?;
            node.register_compiler_abis(entries).await
        },
        |()| Value::Null,
    )
    .await
}

pub async fn get_compiler_abi(
    State(node): State<Arc<Localnet>>,
    axum::extract::Query(payload): axum::extract::Query<GetCompilerAbiQuery>,
) -> Json<Value> {
    handle_result(
        async move {
            let code_hash = parse_hash_any(&payload.code_hash)?;
            node.get_compiler_abi(code_hash).await
        },
        |res| res.clone().unwrap_or(Value::Null),
    )
    .await
}

fn parse_hash_any(hash: &str) -> anyhow::Result<Hash256> {
    if let Ok(parsed) = Hash256::from_hex(hash) {
        return Ok(parsed);
    }
    if let Ok(parsed) = Hash256::from_base64(hash) {
        return Ok(parsed);
    }
    anyhow::bail!("Invalid hash format")
}
