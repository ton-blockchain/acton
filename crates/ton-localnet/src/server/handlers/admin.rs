use super::utils::handle_result;
use crate::api::toncenter_v2 as v2;
use crate::localnet::{Localnet, LocalnetAccountStateChange, LocalnetMiningMode};
use crate::server::models::{
    ChangeAccountStatePayload, ChangeAccountStateRequest, CodeHashRequest,
    CreateRecoveryPointRequest, ExportRecoveryPointRequest, FaucetRequest, GetApiCallsRequest,
    GetVerifiedSourceRequest, ImportRecoveryPointRequest, IncreaseTimeRequest, MineBlocksRequest,
    RegisterCompilerAbisRequest, RegisterVerifiedSourcesRequest, RevertRecoveryPointRequest,
    SendBocRequest, SetAddressNameRequest, SetMiningModeRequest, SetNetworkConditionsRequest,
    SetNextBlockTimestampRequest, SetShardAccountRequest, SetTimeRequest, StatePathRequest,
};
use crate::server::{
    ApiCallLog, NetworkConditions, NetworkConditionsInfo, StartupWallet, StateSourceInfo,
};
use crate::types::Hash256;
use anyhow::Context;
use axum::{
    Json,
    body::Bytes,
    extract::Query,
    extract::{RawQuery, State},
};
use serde::Serialize;
use serde_json::Value;
use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;

const VERIFIER_SOURCE_URL: &str = "https://verifier.acton.monster/api/v1/verification/source";
const VERIFIER_ABI_URL: &str = "https://verifier.acton.monster/api/v1/abi";
const VERIFIER_REQUEST_TIMEOUT: Duration = Duration::from_secs(8);

pub async fn faucet(
    State(node): State<Arc<Localnet>>,
    Json(payload): Json<FaucetRequest>,
) -> Json<Value> {
    handle_result(
        node.faucet(payload.address, payload.amount),
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
    mining_mode: LocalnetMiningMode,
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
            let clock_info = node.clock_info().await?;
            let mining_mode = node.get_mining_mode().await?;

            Ok(LocalnetAdminStatus {
                uptime_seconds: node.uptime_seconds(),
                last_block_seqno: u64::from(masterchain_info.last.seqno),
                current_unix_time: clock_info.current_unix_time,
                time_offset_seconds: clock_info.time_offset_seconds,
                next_block_timestamp: clock_info.next_block_timestamp,
                mining_mode,
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

pub async fn mine_blocks(State(node): State<Arc<Localnet>>, body: Bytes) -> Json<Value> {
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
) -> Json<Value> {
    handle_result(
        node.set_mining_mode(LocalnetMiningMode {
            skip_empty_blocks: payload.skip_empty_blocks,
        }),
        |res| serde_json::to_value(res).unwrap_or(Value::Null),
    )
    .await
}

pub async fn create_recovery_point(
    State(node): State<Arc<Localnet>>,
    Json(payload): Json<CreateRecoveryPointRequest>,
) -> Json<Value> {
    handle_result(
        node.create_recovery_point(payload.name, payload.force),
        |res| serde_json::to_value(res).unwrap_or(Value::Null),
    )
    .await
}

pub async fn list_recovery_points(State(node): State<Arc<Localnet>>) -> Json<Value> {
    handle_result(node.list_recovery_points(), |res| {
        serde_json::to_value(res).unwrap_or(Value::Null)
    })
    .await
}

pub async fn revert_recovery_point(
    State(node): State<Arc<Localnet>>,
    Json(payload): Json<RevertRecoveryPointRequest>,
) -> Json<Value> {
    handle_result(node.revert_recovery_point(payload.name), |res| {
        serde_json::to_value(res).unwrap_or(Value::Null)
    })
    .await
}

pub async fn export_recovery_point(
    State(node): State<Arc<Localnet>>,
    Json(payload): Json<ExportRecoveryPointRequest>,
) -> Json<Value> {
    handle_result(
        node.export_recovery_point(payload.name, payload.path),
        |res| serde_json::to_value(res).unwrap_or(Value::Null),
    )
    .await
}

pub async fn import_recovery_point(
    State(node): State<Arc<Localnet>>,
    Json(payload): Json<ImportRecoveryPointRequest>,
) -> Json<Value> {
    handle_result(
        node.import_recovery_point(payload.name, payload.path, payload.force),
        |res| serde_json::to_value(res).unwrap_or(Value::Null),
    )
    .await
}

pub async fn increase_time(
    State(node): State<Arc<Localnet>>,
    Json(payload): Json<IncreaseTimeRequest>,
) -> Json<Value> {
    handle_result(node.increase_time(payload.seconds), |res| {
        serde_json::to_value(res).unwrap_or(Value::Null)
    })
    .await
}

pub async fn set_time(
    State(node): State<Arc<Localnet>>,
    Json(payload): Json<SetTimeRequest>,
) -> Json<Value> {
    handle_result(node.set_time(payload.timestamp), |res| {
        serde_json::to_value(res).unwrap_or(Value::Null)
    })
    .await
}

pub async fn set_next_block_timestamp(
    State(node): State<Arc<Localnet>>,
    Json(payload): Json<SetNextBlockTimestampRequest>,
) -> Json<Value> {
    handle_result(node.set_next_block_timestamp(payload.timestamp), |res| {
        serde_json::to_value(res).unwrap_or(Value::Null)
    })
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

pub async fn change_account_state(
    State(node): State<Arc<Localnet>>,
    Json(payload): Json<ChangeAccountStateRequest>,
) -> Json<Value> {
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
) -> Json<Value> {
    handle_result(
        node.send_internal_boc(payload.boc),
        v2::map_send_internal_message,
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
                .map(|entry| compiler_abi_registration_entry(entry.abi))
                .collect::<anyhow::Result<Vec<_>>>()?;
            node.register_compiler_abis(entries).await
        },
        |()| Value::Null,
    )
    .await
}

fn compiler_abi_registration_entry(abi: Value) -> anyhow::Result<(Hash256, Value)> {
    let code_hash = abi
        .get("code_hashes")
        .and_then(Value::as_array)
        .and_then(|code_hashes| code_hashes.iter().find_map(Value::as_str))
        .ok_or_else(|| anyhow::anyhow!("compiler ABI registration requires abi.code_hashes[0]"))?;
    Ok((parse_hash_any(code_hash)?, abi))
}

pub async fn list_compiler_abis(State(node): State<Arc<Localnet>>) -> Json<Value> {
    handle_result(node.list_compiler_abis(), |entries| {
        serde_json::to_value(
            entries
                .iter()
                .map(|(code_hash, abi)| {
                    serde_json::json!({
                        "codeHash": code_hash,
                        "abi": abi,
                        "savedAt": 0_u64,
                    })
                })
                .collect::<Vec<_>>(),
        )
        .unwrap_or(Value::Null)
    })
    .await
}

pub async fn delete_compiler_abi(
    State(node): State<Arc<Localnet>>,
    Json(payload): Json<CodeHashRequest>,
) -> Json<Value> {
    handle_result(node.delete_compiler_abi(payload.code_hash), |()| {
        Value::Null
    })
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

pub async fn register_verified_sources(
    State(node): State<Arc<Localnet>>,
    Json(payload): Json<RegisterVerifiedSourcesRequest>,
) -> Json<Value> {
    handle_result(
        async move {
            let entries = payload
                .entries
                .into_iter()
                .map(|entry| {
                    let code_hash = parse_hash_any(&entry.code_hash)?;
                    let compiler_abis =
                        compiler_abis_from_registered_source(&code_hash, &entry.source);
                    Ok((code_hash, entry.source, compiler_abis))
                })
                .collect::<anyhow::Result<Vec<_>>>()?;
            let mut compiler_abis = Vec::new();
            let sources = entries
                .into_iter()
                .map(|(code_hash, source, source_compiler_abis)| {
                    compiler_abis.extend(source_compiler_abis);
                    (code_hash, source)
                })
                .collect();
            node.register_verified_sources(sources).await?;
            node.register_compiler_abis(compiler_abis).await
        },
        |()| Value::Null,
    )
    .await
}

pub async fn get_registered_verified_source(
    State(node): State<Arc<Localnet>>,
    Query(payload): Query<GetVerifiedSourceRequest>,
) -> Json<Value> {
    handle_result(
        async move {
            let source = node
                .get_registered_verified_source(payload.address.clone(), payload.code_hash.clone())
                .await?;
            Ok(source.unwrap_or_else(|| unverified_source_response(&payload)))
        },
        Clone::clone,
    )
    .await
}

pub async fn list_verified_sources(State(node): State<Arc<Localnet>>) -> Json<Value> {
    handle_result(node.list_verified_sources(), |entries| {
        serde_json::to_value(
            entries
                .iter()
                .map(|(code_hash, source)| {
                    serde_json::json!({
                        "codeHash": code_hash,
                        "source": source,
                        "savedAt": 0_u64,
                    })
                })
                .collect::<Vec<_>>(),
        )
        .unwrap_or(Value::Null)
    })
    .await
}

pub async fn delete_verified_source(
    State(node): State<Arc<Localnet>>,
    Json(payload): Json<CodeHashRequest>,
) -> Json<Value> {
    handle_result(node.delete_verified_source(payload.code_hash), |()| {
        Value::Null
    })
    .await
}

pub async fn get_verified_source(
    State(node): State<Arc<Localnet>>,
    Query(payload): Query<GetVerifiedSourceRequest>,
) -> Json<Value> {
    handle_result(
        async move {
            if let Some(source) = node
                .get_registered_verified_source(payload.address.clone(), payload.code_hash.clone())
                .await?
            {
                return Ok(source);
            }

            let value = fetch_verified_source(payload).await?;
            let entries = match fetch_verified_compiler_abis(&value).await {
                Ok(entries) => entries,
                Err(error) => {
                    tracing::warn!(?error, "failed to fetch verifier compiler ABI records");
                    verified_source_compiler_abis(&value)
                }
            };
            if !entries.is_empty()
                && let Err(error) = node.register_compiler_abis(entries).await
            {
                tracing::warn!(?error, "failed to register verifier compiler ABI");
            }

            Ok(value)
        },
        Clone::clone,
    )
    .await
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

    let response = reqwest::Client::builder()
        .timeout(VERIFIER_REQUEST_TIMEOUT)
        .build()?
        .get(url)
        .send()
        .await?;
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

fn unverified_source_response(payload: &GetVerifiedSourceRequest) -> Value {
    serde_json::json!({
        "code_hash": payload.code_hash.as_deref().and_then(|code_hash| {
            parse_hash_any(code_hash).ok().map(|hash| hash.to_hex())
        }),
        "verified": false,
        "bundles": [],
    })
}

async fn fetch_verified_compiler_abis(source: &Value) -> anyhow::Result<Vec<(Hash256, Value)>> {
    let code_hash = source
        .get("code_hash")
        .and_then(Value::as_str)
        .context("Verifier source response does not contain code_hash")?;

    let mut url = reqwest::Url::parse(VERIFIER_ABI_URL)?;
    url.query_pairs_mut().append_pair("code_hash", code_hash);

    let response = reqwest::Client::builder()
        .timeout(VERIFIER_REQUEST_TIMEOUT)
        .build()?
        .get(url)
        .send()
        .await?;
    let status = response.status();
    let body = response.text().await?;
    let value = serde_json::from_str::<Value>(&body).unwrap_or(Value::String(body));

    if !status.is_success() {
        let message = value.get("error").and_then(Value::as_str).map_or_else(
            || format!("Verifier ABI request failed with status {status}"),
            ToOwned::to_owned,
        );
        anyhow::bail!("{message}");
    }

    Ok(abi_response_compiler_abis(&value))
}

fn abi_response_compiler_abis(value: &Value) -> Vec<(Hash256, Value)> {
    let Some(items) = value.get("items").and_then(Value::as_array) else {
        return Vec::new();
    };

    items
        .iter()
        .filter_map(|item| {
            let code_hash = item.get("code_hash").and_then(Value::as_str)?;
            let code_hash = parse_hash_any(code_hash).ok()?;
            let compiler_abi = item.get("abi").filter(|abi| abi.is_object())?.clone();
            Some((
                code_hash,
                compiler_abi_payload_value(&code_hash, compiler_abi),
            ))
        })
        .collect()
}

fn non_empty_text(value: Option<String>) -> Option<String> {
    value.filter(|value| !value.trim().is_empty())
}

fn verified_source_compiler_abis(value: &Value) -> Vec<(Hash256, Value)> {
    let Some(code_hash) = value.get("code_hash").and_then(Value::as_str) else {
        return Vec::new();
    };
    let Ok(code_hash) = parse_hash_any(code_hash) else {
        return Vec::new();
    };
    let Some(bundles) = value.get("bundles").and_then(Value::as_array) else {
        return Vec::new();
    };

    bundles
        .iter()
        .filter_map(compiler_abi_from_verified_source_bundle)
        .map(|compiler_abi| {
            (
                code_hash,
                compiler_abi_payload_value(&code_hash, compiler_abi),
            )
        })
        .collect()
}

fn compiler_abis_from_registered_source(
    code_hash: &Hash256,
    value: &Value,
) -> Vec<(Hash256, Value)> {
    let Some(bundles) = value.get("bundles").and_then(Value::as_array) else {
        return Vec::new();
    };

    bundles
        .iter()
        .filter_map(compiler_abi_from_verified_source_bundle)
        .map(|compiler_abi| {
            (
                *code_hash,
                compiler_abi_payload_value(code_hash, compiler_abi),
            )
        })
        .collect()
}

fn compiler_abi_payload_value(code_hash: &Hash256, compiler_abi: Value) -> Value {
    let display_name = compiler_abi
        .get("contract_name")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .map(ToOwned::to_owned);

    serde_json::json!({
        "compiler_abi": compiler_abi,
        "display_name": display_name,
        "code_hashes": [code_hash.to_hex()],
        "links": [],
    })
}

fn compiler_abi_from_verified_source_bundle(bundle: &Value) -> Option<Value> {
    if let Some(compiler_abi) = bundle
        .get("compiler_abi")
        .filter(|compiler_abi| compiler_abi.is_object())
        .cloned()
    {
        return Some(compiler_abi);
    }

    bundle
        .get("files")
        .and_then(Value::as_array)?
        .iter()
        .find_map(compiler_abi_from_verified_source_file)
}

fn compiler_abi_from_verified_source_file(file: &Value) -> Option<Value> {
    let path = file.get("path").and_then(Value::as_str)?;
    if !path.ends_with(".abi.json") {
        return None;
    }

    let content = verified_source_file_content(file)?;
    let compiler_abi = serde_json::from_str::<Value>(&content).ok()?;
    compiler_abi.is_object().then_some(compiler_abi)
}

fn verified_source_file_content(file: &Value) -> Option<String> {
    file.get("content")
        .and_then(Value::as_str)
        .map(str::to_owned)
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn extracts_compiler_abi_from_verified_source_file_text() {
        let code_hash = Hash256([0x42; 32]);
        let source = json!({
            "code_hash": code_hash.to_hex(),
            "bundles": [
                {
                    "files": [
                        {
                            "path": "output/counter.abi.json",
                            "content": r#"{"contract_name":"Counter","get_methods":[]}"#
                        }
                    ]
                }
            ]
        });

        let entries = verified_source_compiler_abis(&source);

        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].0, code_hash);
        assert_eq!(entries[0].1["display_name"], "Counter");
        assert_eq!(entries[0].1["compiler_abi"]["contract_name"], "Counter");
        assert_eq!(entries[0].1["code_hashes"], json!([code_hash.to_hex()]));
    }

    #[test]
    fn extracts_compiler_abi_from_abi_response() {
        let code_hash = Hash256([0x24; 32]);
        let abi_response = json!({
            "items": [
                {
                    "code_hash": code_hash.to_hex(),
                    "abi": {
                        "contract_name": "Wallet",
                        "get_methods": []
                    }
                }
            ]
        });

        let entries = abi_response_compiler_abis(&abi_response);

        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].0, code_hash);
        assert_eq!(entries[0].1["display_name"], "Wallet");
        assert_eq!(entries[0].1["compiler_abi"]["contract_name"], "Wallet");
        assert_eq!(entries[0].1["code_hashes"], json!([code_hash.to_hex()]));
    }

    #[test]
    fn ignores_missing_or_invalid_verified_source_abi() {
        let code_hash = Hash256([0x11; 32]);
        let source = json!({
            "code_hash": code_hash.to_hex(),
            "bundles": [
                {
                    "files": [
                        {
                            "path": "output/broken.abi.json",
                            "content": "not json"
                        },
                        {
                            "path": "src/main.tolk",
                            "content": "fun main() {}"
                        }
                    ]
                }
            ]
        });

        assert!(verified_source_compiler_abis(&source).is_empty());
    }
}
