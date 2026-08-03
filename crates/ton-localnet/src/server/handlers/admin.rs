use super::utils::handle_result;
use crate::api::toncenter_v2 as v2;
use crate::localnet::{Localnet, LocalnetAccountStateChange, LocalnetMiningMode};
use crate::server::models::{
    ChangeAccountStatePayload, ChangeAccountStateRequest, CheckpointRequest,
    CreateCheckpointRequest, FaucetRequest, GetApiCallsRequest, GetVerifiedSourceRequest,
    ImportCheckpointQuery, IncreaseTimeRequest, JettonFaucetRequest, MineBlocksRequest,
    SetMiningModeRequest, SetNetworkConditionsRequest, SetNextBlockTimestampRequest,
    SetShardAccountRequest, SetTimeRequest,
};
use crate::server::{
    ApiCallLog, NetworkConditions, NetworkConditionsInfo, ServerState, StartupAccount,
    StateSourceInfo,
};
use crate::types::Hash256;
use axum::{
    Json,
    body::Bytes,
    extract::Query,
    extract::State,
    http::header::{CONTENT_DISPOSITION, CONTENT_TYPE},
    response::{IntoResponse, Response},
};
use serde::Serialize;
use serde_json::Value;
use std::sync::Arc;
use std::time::Duration;
use ton_api::toncenter::v2::requests::SendBocRequest;

const VERIFIER_SOURCE_URL: &str = "https://verifier.acton.monster/api/v1/verification/source";
const VERIFIER_REQUEST_TIMEOUT: Duration = Duration::from_secs(8);

const fn user_agent() -> &'static str {
    concat!("acton/", env!("CARGO_PKG_VERSION"))
}

fn build_verifier_http_client() -> Result<reqwest::Client, reqwest::Error> {
    reqwest::Client::builder()
        .timeout(VERIFIER_REQUEST_TIMEOUT)
        .user_agent(user_agent())
        .build()
}

pub async fn faucet(
    State(node): State<Arc<Localnet>>,
    Json(payload): Json<FaucetRequest>,
) -> Response {
    handle_result(
        node.faucet(payload.address, payload.amount),
        v2::map_send_internal_message,
    )
    .await
}

pub async fn jetton_faucet(
    State(node): State<Arc<Localnet>>,
    Json(payload): Json<JettonFaucetRequest>,
) -> Response {
    handle_result(
        node.jetton_faucet(payload.address, payload.jetton_master, payload.amount),
        v2::map_send_internal_message,
    )
    .await
}

#[derive(Serialize)]
struct LocalnetAdminStatus {
    uptime_seconds: u64,
    last_block_seqno: u64,
    current_unix_time: u32,
    time_offset_seconds: i64,
    next_block_timestamp: Option<u32>,
    auto_mining: bool,
    block_interval_ms: u64,
    rate_limit_rps: Option<u32>,
    mining_mode: LocalnetMiningMode,
    #[serde(flatten)]
    state_source: StateSourceInfo,
    network_conditions: NetworkConditionsInfo,
}

pub async fn get_status(State(state): State<ServerState>) -> Response {
    let ServerState {
        node,
        network_conditions,
        rate_limit_rps,
        ..
    } = state;
    handle_result(
        async move {
            let masterchain_info = node.get_masterchain_info().await?;
            let clock_info = node.clock_info().await?;
            let mining_mode = node.get_mining_mode().await?;
            let state_source = node.state_source().await?;

            Ok(LocalnetAdminStatus {
                uptime_seconds: node.uptime_seconds(),
                last_block_seqno: u64::from(masterchain_info.last.seqno),
                current_unix_time: clock_info.current_unix_time,
                time_offset_seconds: clock_info.time_offset_seconds,
                next_block_timestamp: clock_info.next_block_timestamp,
                auto_mining: node.auto_mining(),
                block_interval_ms: node.block_interval_ms(),
                rate_limit_rps,
                mining_mode,
                state_source: StateSourceInfo::from(&state_source),
                network_conditions: network_conditions.info(),
            })
        },
        |res| serde_json::to_value(res).unwrap_or(Value::Null),
    )
    .await
}

pub async fn get_startup_accounts(
    State(startup_accounts): State<Arc<Vec<StartupAccount>>>,
) -> Response {
    handle_result(
        async move { Ok::<_, anyhow::Error>(startup_accounts.as_ref().clone()) },
        |res| serde_json::to_value(res).unwrap_or(Value::Null),
    )
    .await
}

pub async fn set_network_conditions(
    State(network_conditions): State<NetworkConditions>,
    Json(payload): Json<SetNetworkConditionsRequest>,
) -> Response {
    network_conditions.set_response_delay_ms(payload.response_delay_ms);
    handle_result(
        async move { Ok::<_, anyhow::Error>(network_conditions.info()) },
        |res| serde_json::to_value(res).unwrap_or(Value::Null),
    )
    .await
}

pub async fn mine_blocks(State(node): State<Arc<Localnet>>, body: Bytes) -> Response {
    handle_result(
        async move {
            let payload = if body.is_empty() {
                MineBlocksRequest::default()
            } else {
                serde_json::from_slice::<MineBlocksRequest>(&body)
                    .map_err(|e| anyhow::anyhow!("Invalid mine request JSON: {e}"))?
            };
            node.mine_blocks(payload.blocks.unwrap_or(1)).await
        },
        |res| serde_json::to_value(res).unwrap_or(Value::Null),
    )
    .await
}

pub async fn set_mining_mode(
    State(node): State<Arc<Localnet>>,
    Json(payload): Json<SetMiningModeRequest>,
) -> Response {
    handle_result(
        node.set_mining_mode(LocalnetMiningMode {
            skip_empty_blocks: payload.skip_empty_blocks,
        }),
        |res| serde_json::to_value(res).unwrap_or(Value::Null),
    )
    .await
}

pub async fn create_checkpoint(
    State(node): State<Arc<Localnet>>,
    Json(payload): Json<CreateCheckpointRequest>,
) -> Response {
    handle_result(node.create_checkpoint(payload.name, payload.force), |res| {
        serde_json::to_value(res).unwrap_or(Value::Null)
    })
    .await
}

pub async fn list_checkpoints(State(node): State<Arc<Localnet>>) -> Response {
    handle_result(node.list_checkpoints(), |res| {
        serde_json::to_value(res).unwrap_or(Value::Null)
    })
    .await
}

pub async fn restore_checkpoint(
    State(node): State<Arc<Localnet>>,
    Json(payload): Json<CheckpointRequest>,
) -> Response {
    handle_result(node.restore_checkpoint(payload.name), |res| {
        serde_json::to_value(res).unwrap_or(Value::Null)
    })
    .await
}

pub async fn delete_checkpoint(
    State(node): State<Arc<Localnet>>,
    Json(payload): Json<CheckpointRequest>,
) -> Response {
    handle_result(node.delete_checkpoint(payload.name), |res| {
        serde_json::to_value(res).unwrap_or(Value::Null)
    })
    .await
}

pub async fn clear_checkpoints(State(node): State<Arc<Localnet>>) -> Response {
    handle_result(
        node.clear_checkpoints(),
        |deleted| serde_json::json!({ "deleted": deleted }),
    )
    .await
}

pub async fn export_checkpoint(
    State(node): State<Arc<Localnet>>,
    Query(payload): Query<CheckpointRequest>,
) -> Response {
    json_download_response(
        node.export_checkpoint(payload.name).await,
        "attachment; filename=acton-localnet-checkpoint.json",
    )
    .await
}

pub async fn import_checkpoint(
    State(node): State<Arc<Localnet>>,
    Query(payload): Query<ImportCheckpointQuery>,
    body: Bytes,
) -> Response {
    handle_result(
        node.import_checkpoint(payload.name, body.to_vec(), payload.force),
        |res| serde_json::to_value(res).unwrap_or(Value::Null),
    )
    .await
}

pub async fn increase_time(
    State(node): State<Arc<Localnet>>,
    Json(payload): Json<IncreaseTimeRequest>,
) -> Response {
    handle_result(node.increase_time(payload.seconds), |res| {
        serde_json::to_value(res).unwrap_or(Value::Null)
    })
    .await
}

pub async fn set_time(
    State(node): State<Arc<Localnet>>,
    Json(payload): Json<SetTimeRequest>,
) -> Response {
    handle_result(node.set_time(payload.timestamp), |res| {
        serde_json::to_value(res).unwrap_or(Value::Null)
    })
    .await
}

pub async fn set_next_block_timestamp(
    State(node): State<Arc<Localnet>>,
    Json(payload): Json<SetNextBlockTimestampRequest>,
) -> Response {
    handle_result(node.set_next_block_timestamp(payload.timestamp), |res| {
        serde_json::to_value(res).unwrap_or(Value::Null)
    })
    .await
}

pub async fn get_api_calls(
    State(api_calls): State<ApiCallLog>,
    Query(payload): Query<GetApiCallsRequest>,
) -> Response {
    handle_result(
        async move { Ok::<_, anyhow::Error>(api_calls.snapshot(payload.limit)) },
        |res| serde_json::to_value(res).unwrap_or(Value::Null),
    )
    .await
}

pub async fn dump_state(State(node): State<Arc<Localnet>>) -> Response {
    json_download_response(
        node.dump_state().await,
        "attachment; filename=acton-localnet-state.json",
    )
    .await
}

pub async fn load_state(State(node): State<Arc<Localnet>>, body: Bytes) -> Response {
    handle_result(node.load_state(body.to_vec()), |()| Value::Null).await
}

async fn json_download_response(
    result: anyhow::Result<Vec<u8>>,
    content_disposition: &'static str,
) -> Response {
    match result {
        Ok(json) => (
            [
                (CONTENT_TYPE, "application/json"),
                (CONTENT_DISPOSITION, content_disposition),
            ],
            json,
        )
            .into_response(),
        Err(error) => handle_result(async { Err::<(), _>(error) }, |()| Value::Null).await,
    }
}

pub async fn set_shard_account(
    State(node): State<Arc<Localnet>>,
    Json(payload): Json<SetShardAccountRequest>,
) -> Response {
    handle_result(
        node.set_shard_account(payload.address, payload.shard_account),
        |()| Value::Null,
    )
    .await
}

pub async fn change_account_state(
    State(node): State<Arc<Localnet>>,
    Json(payload): Json<ChangeAccountStateRequest>,
) -> Response {
    handle_result(
        async move {
            let change = parse_account_state_change(payload.state)?;
            node.change_account_state(payload.address, change, payload.mine)
                .await
        },
        |()| Value::Null,
    )
    .await
}

pub async fn send_internal_message(
    State(node): State<Arc<Localnet>>,
    Json(payload): Json<SendBocRequest>,
) -> Response {
    handle_result(
        node.send_internal_boc(payload.boc),
        v2::map_send_internal_message,
    )
    .await
}

pub async fn get_verified_source(Query(payload): Query<GetVerifiedSourceRequest>) -> Response {
    handle_result(fetch_verified_source(payload), Clone::clone).await
}

async fn fetch_verified_source(payload: GetVerifiedSourceRequest) -> anyhow::Result<Value> {
    let address = non_empty_text(payload.address);
    let code_hash = non_empty_text(payload.code_hash);
    if address.is_none() && code_hash.is_none() {
        anyhow::bail!("Provide address or code_hash");
    }

    let mut url = reqwest::Url::parse(VERIFIER_SOURCE_URL)?;
    {
        let mut query = url.query_pairs_mut();
        if let Some(address) = address {
            query.append_pair("address", &address);
        }
        if let Some(code_hash) = code_hash {
            query.append_pair("code_hash", &code_hash);
        }
    }

    let response = build_verifier_http_client()?.get(url).send().await?;
    let status = response.status();
    let body = response.text().await?;
    let value = serde_json::from_str::<Value>(&body).unwrap_or(Value::String(body));

    if !status.is_success() {
        let message = value.get("error").and_then(Value::as_str).map_or_else(
            || format!("Verifier request failed with status {status}"),
            ToOwned::to_owned,
        );
        anyhow::bail!("{message}");
    }

    Ok(value)
}

fn non_empty_text(value: Option<String>) -> Option<String> {
    value.filter(|value| !value.trim().is_empty())
}

fn parse_hash_any(hash: &str) -> anyhow::Result<Hash256> {
    hash.parse()
}

fn parse_account_state_change(
    payload: ChangeAccountStatePayload,
) -> anyhow::Result<LocalnetAccountStateChange> {
    match payload {
        ChangeAccountStatePayload::Nonexist => Ok(LocalnetAccountStateChange::Nonexist),
        ChangeAccountStatePayload::Uninit { balance } => Ok(LocalnetAccountStateChange::Uninit {
            balance: parse_optional_balance(balance)?,
        }),
        ChangeAccountStatePayload::Frozen {
            source,
            frozen_hash,
            balance,
        } => match (source.as_deref(), frozen_hash.as_deref()) {
            (Some("current"), None) => {
                if balance.is_some() {
                    anyhow::bail!("`balance` cannot be used with frozen `source: current`");
                }
                Ok(LocalnetAccountStateChange::FrozenFromCurrent)
            }
            (Some("current"), Some(_)) => {
                anyhow::bail!("`frozen_hash` cannot be used with frozen `source: current`")
            }
            (None, Some(hash)) => Ok(LocalnetAccountStateChange::Frozen {
                frozen_hash: parse_hash_any(hash)?,
                balance: parse_optional_balance(balance)?,
            }),
            (Some(other), _) => anyhow::bail!(
                "Unsupported frozen account state source `{other}`; supported value is `current`"
            ),
            (None, None) => anyhow::bail!(
                "Frozen account state requires either `source: current` or `frozen_hash`"
            ),
        },
    }
}

fn parse_optional_balance(balance: Option<String>) -> anyhow::Result<u128> {
    let Some(balance) = balance else {
        return Ok(0);
    };
    balance
        .parse::<u128>()
        .map_err(|_| anyhow::anyhow!("Invalid balance: {balance}"))
}
