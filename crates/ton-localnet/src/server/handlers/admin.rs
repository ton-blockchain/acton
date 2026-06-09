use super::utils::handle_result;
use crate::api::toncenter_v2 as v2;
use crate::localnet::Localnet;
use crate::server::models::{
    FaucetRequest, GetApiCallsRequest, RegisterCompilerAbisRequest, SendBocRequest,
    SetAddressNameRequest, SetNetworkConditionsRequest, SetShardAccountRequest, StatePathRequest,
};
use crate::server::{
    ApiCallLog, NetworkConditions, NetworkConditionsInfo, StartupWallet, StateSourceInfo,
};
use crate::types::Hash256;
use axum::{
    Json,
    extract::Query,
    extract::{RawQuery, State},
};
use serde::Serialize;
use serde_json::Value;
use std::collections::BTreeMap;
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

#[derive(Serialize)]
struct LocalnetAdminStatus {
    uptime_seconds: u64,
    last_block_seqno: u64,
    #[serde(flatten)]
    state_source: StateSourceInfo,
    network_conditions: NetworkConditionsInfo,
}

pub async fn get_status(
    State(node): State<Arc<Localnet>>,
    State(state_source): State<Arc<StateSourceInfo>>,
    State(network_conditions): State<NetworkConditions>,
) -> Json<Value> {
    handle_result(
        async move {
            let masterchain_info = node.get_masterchain_info().await?;

            Ok(LocalnetAdminStatus {
                uptime_seconds: node.uptime_seconds(),
                last_block_seqno: u64::from(masterchain_info.last.seqno),
                state_source: state_source.as_ref().clone(),
                network_conditions: network_conditions.info(),
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

pub async fn set_network_conditions(
    State(network_conditions): State<NetworkConditions>,
    Json(payload): Json<SetNetworkConditionsRequest>,
) -> Json<Value> {
    network_conditions.set_response_delay_ms(payload.response_delay_ms);
    handle_result(
        async move { Ok::<_, anyhow::Error>(network_conditions.info()) },
        |res| serde_json::to_value(res).unwrap_or(Value::Null),
    )
    .await
}

pub async fn get_api_calls(
    State(api_calls): State<ApiCallLog>,
    Query(payload): Query<GetApiCallsRequest>,
) -> Json<Value> {
    handle_result(
        async move { Ok::<_, anyhow::Error>(api_calls.snapshot(payload.limit)) },
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

pub async fn set_shard_account(
    State(node): State<Arc<Localnet>>,
    Json(payload): Json<SetShardAccountRequest>,
) -> Json<Value> {
    handle_result(
        node.set_shard_account(payload.address, payload.shard_account),
        |()| Value::Null,
    )
    .await
}

pub async fn send_internal_message(
    State(node): State<Arc<Localnet>>,
    Json(payload): Json<SendBocRequest>,
) -> Json<Value> {
    handle_result(
        node.send_internal_boc(payload.boc),
        v2::map_send_boc_return_hash,
    )
    .await
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
    RawQuery(query): RawQuery,
) -> Json<Value> {
    let addresses = query
        .as_deref()
        .map(|query| {
            url::form_urlencoded::parse(query.as_bytes())
                .filter_map(|(key, value)| (key == "address").then(|| value.into_owned()))
                .collect()
        })
        .unwrap_or_default();

    handle_result(node.get_address_names(addresses), |entries| {
        serde_json::to_value(entries.iter().cloned().collect::<BTreeMap<_, _>>())
            .unwrap_or(Value::Null)
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
    RawQuery(query): RawQuery,
) -> Json<Value> {
    let code_hashes = query
        .as_deref()
        .map(|query| {
            url::form_urlencoded::parse(query.as_bytes())
                .filter_map(|(key, value)| (key == "code_hash").then(|| value.into_owned()))
                .collect()
        })
        .unwrap_or_default();

    handle_result(node.get_compiler_abis(code_hashes), |entries| {
        serde_json::to_value(entries.iter().cloned().collect::<BTreeMap<_, _>>())
            .unwrap_or(Value::Null)
    })
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
