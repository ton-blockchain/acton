use super::utils::handle_result;
use crate::localnet::Localnet;
use crate::node;
use crate::server::StartupWallet;
use crate::server::models::{
    FaucetRequest, GetAddressNameQuery, GetCompilerAbiQuery, RegisterCompilerAbisRequest,
    SetAddressNameRequest, StatePathRequest,
};
use crate::types::Hash256;
use axum::{Json, extract::State};
use serde::Serialize;
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

#[derive(Serialize)]
struct LocalnetAdminStatus {
    uptime_seconds: u64,
    last_block_seqno: u64,
    state_source: String,
    fork_network: Option<String>,
    fork_block_number: Option<u64>,
}

pub async fn get_status(State(node): State<Arc<Localnet>>) -> Json<Value> {
    handle_result(
        async move {
            let masterchain_info = node.get_masterchain_info().await?;
            let state_source = node.get_state_source().await?;
            let (state_source_name, fork_network, fork_block_number) = match state_source {
                node::StateSource::Local => ("local".to_owned(), None, None),
                node::StateSource::Remote(provider) => (
                    "remote".to_owned(),
                    Some(provider.network.to_string()),
                    provider.fork_block_number,
                ),
            };

            Ok(LocalnetAdminStatus {
                uptime_seconds: node.uptime_seconds(),
                last_block_seqno: u64::from(masterchain_info.last.seqno),
                state_source: state_source_name,
                fork_network,
                fork_block_number,
            })
        },
        |res| serde_json::to_value(res).unwrap_or(Value::Null),
    )
    .await
}

pub async fn get_startup_wallets(
    State(startup_wallets): State<Arc<Vec<StartupWallet>>>,
) -> Json<Value> {
    handle_result(
        async move { Ok::<_, anyhow::Error>(startup_wallets.as_ref().clone()) },
        |res| serde_json::to_value(res).unwrap_or(Value::Null),
    )
    .await
}

pub async fn dump_state(
    State(node): State<Arc<Localnet>>,
    Json(payload): Json<StatePathRequest>,
) -> Json<Value> {
    handle_result(node.dump_state(payload.path), |()| Value::Null).await
}

pub async fn load_state(
    State(node): State<Arc<Localnet>>,
    Json(payload): Json<StatePathRequest>,
) -> Json<Value> {
    handle_result(node.load_state(payload.path), |()| Value::Null).await
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
