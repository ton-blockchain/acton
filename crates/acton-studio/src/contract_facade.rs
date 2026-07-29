use std::collections::{BTreeMap, BTreeSet};
use std::str::FromStr;
use std::time::{SystemTime, UNIX_EPOCH};

use axum::Json;
use axum::body::to_bytes;
use axum::extract::Request;
use axum::http::{HeaderValue, Method, StatusCode};
use axum::response::{IntoResponse, Response};
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::Value;
use ton::ton_core::cell::TonHash;
use ton::ton_core::types::TonAddress;
use ton_api::toncenter::v2::{TonlibErrorResponse, TonlibResponse};
use ton_api::toncenter::v3::responses::{AccountStateFull, AccountStatesResponse};

use crate::contract_registry::{
    ArtifactIdRequest, CodeHashRequest, ContractArtifact, ContractListEntry, ContractRegistryError,
    ContractRegistryStore, ContractSourceKind, DeleteContractRequest, GetVerifiedSourceRequest,
    RegisterCompilerAbisRequest, RegisterContractRequest, RegisterVerifiedSourcesRequest,
    RegistrySnapshot, SavedVerifiedSource, SetAddressNameRequest,
};
use crate::{EnvironmentConfig, StudioEnvironment, apply_environment_upstream_auth};

const MAX_EXTENSION_BODY_BYTES: usize = 64 * 1024 * 1024;
const MAX_ACCOUNT_STATES_BATCH_SIZE: usize = 50;

#[derive(Clone, Copy)]
enum ContractRoute {
    GetAddressName,
    SetAddressName,
    ListContracts,
    RegisterContract,
    DeleteContract,
    GetCompilerAbi,
    ListCompilerAbis,
    RegisterCompilerAbis,
    DeleteCompilerAbi,
    GetRegisteredVerifiedSource,
    ListVerifiedSources,
    RegisterVerifiedSources,
    DeleteVerifiedSource,
    DeleteVerifiedSourceArtifact,
}

impl ContractRoute {
    fn accepts(self, method: &Method) -> bool {
        match self {
            Self::GetAddressName
            | Self::ListContracts
            | Self::GetCompilerAbi
            | Self::ListCompilerAbis
            | Self::GetRegisteredVerifiedSource
            | Self::ListVerifiedSources => method == Method::GET,
            Self::SetAddressName
            | Self::RegisterContract
            | Self::DeleteContract
            | Self::RegisterCompilerAbis
            | Self::DeleteCompilerAbi
            | Self::RegisterVerifiedSources
            | Self::DeleteVerifiedSource
            | Self::DeleteVerifiedSourceArtifact => method == Method::POST,
        }
    }
}

pub(crate) fn handles(_method: &Method, path: &str) -> bool {
    contract_route(path).is_some()
}

pub(crate) async fn handle(
    store: &ContractRegistryStore,
    http_client: &reqwest::Client,
    environment: &StudioEnvironment,
    toncenter_api_key: Option<&HeaderValue>,
    path: &str,
    request: Request,
) -> Response {
    let Some(route) = route(request.method(), path) else {
        return ContractFacadeError::MethodNotAllowed.into_response();
    };
    match handle_route(
        store,
        http_client,
        environment,
        toncenter_api_key,
        route,
        request,
    )
    .await
    {
        Ok(response) => response,
        Err(error) => error.into_response(),
    }
}

async fn handle_route(
    store: &ContractRegistryStore,
    http_client: &reqwest::Client,
    environment: &StudioEnvironment,
    toncenter_api_key: Option<&HeaderValue>,
    route: ContractRoute,
    request: Request,
) -> Result<Response, ContractFacadeError> {
    match route {
        ContractRoute::GetAddressName => {
            let query = query_pairs(&request);
            let snapshot = store.snapshot(&environment.id).await?;
            let names = query
                .iter()
                .filter(|(key, _)| key == "address")
                .map(|(_, address)| {
                    let name = canonical_address(address)
                        .ok()
                        .and_then(|address| snapshot.address_name(&address))
                        .map(ToOwned::to_owned);
                    (address.clone(), name)
                })
                .collect::<BTreeMap<_, _>>();
            Ok(success(names))
        }
        ContractRoute::SetAddressName => {
            let payload: SetAddressNameRequest = json_body(request).await?;
            let canonical = canonical_address(&payload.address)?;
            store
                .set_address_name(&environment.id, canonical, payload.name)
                .await?;
            Ok(success(Value::Null))
        }
        ContractRoute::ListContracts => {
            let contracts =
                list_contracts(store, http_client, environment, toncenter_api_key).await?;
            Ok(success(contracts))
        }
        ContractRoute::RegisterContract => {
            let payload: RegisterContractRequest = json_body(request).await?;
            let canonical = canonical_address(&payload.address)?;
            let state =
                fetch_single_account_state(http_client, environment, toncenter_api_key, &canonical)
                    .await?;
            ensure_active_contract(&state, &payload.address)?;
            let display_address = display_address(&canonical)?;
            store
                .register_contract(
                    &environment.id,
                    canonical.clone(),
                    display_address,
                    payload.name,
                )
                .await?;
            let snapshot = store.snapshot(&environment.id).await?;
            let mut contract = contract_from_account_state(
                &state,
                &snapshot,
                registered_source_kind(environment),
            )?;
            enrich_contract(&mut contract, &snapshot);
            Ok(success(contract))
        }
        ContractRoute::DeleteContract => {
            let payload: DeleteContractRequest = json_body(request).await?;
            let canonical = canonical_address(&payload.address)?;
            store.delete_contract(&environment.id, &canonical).await?;
            Ok(success(Value::Null))
        }
        ContractRoute::GetCompilerAbi => {
            let snapshot = store.snapshot(&environment.id).await?;
            let result = query_pairs(&request)
                .into_iter()
                .filter(|(key, _)| key == "code_hash")
                .map(|(_, code_hash)| {
                    let abi = snapshot
                        .compiler_abi(&code_hash)
                        .map(|saved| saved.abi.clone());
                    (code_hash, abi)
                })
                .collect::<BTreeMap<_, _>>();
            Ok(success(result))
        }
        ContractRoute::ListCompilerAbis => {
            let snapshot = store.snapshot(&environment.id).await?;
            let mut seen = Vec::<Value>::new();
            let mut entries = snapshot
                .compiler_abis
                .values()
                .filter(|entry| {
                    if seen.contains(&entry.abi) {
                        false
                    } else {
                        seen.push(entry.abi.clone());
                        true
                    }
                })
                .cloned()
                .collect::<Vec<_>>();
            entries.sort_by(|left, right| {
                right
                    .saved_at
                    .cmp(&left.saved_at)
                    .then_with(|| left.code_hash.cmp(&right.code_hash))
            });
            Ok(success(entries))
        }
        ContractRoute::RegisterCompilerAbis => {
            let payload: RegisterCompilerAbisRequest = json_body(request).await?;
            store
                .register_compiler_abis(&environment.id, &payload.entries)
                .await?;
            Ok(success(Value::Null))
        }
        ContractRoute::DeleteCompilerAbi => {
            let payload: CodeHashRequest = json_body(request).await?;
            store
                .delete_compiler_abi(&environment.id, &payload.code_hash)
                .await?;
            Ok(success(Value::Null))
        }
        ContractRoute::GetRegisteredVerifiedSource => {
            let payload = query::<GetVerifiedSourceRequest>(&request)?;
            let snapshot = store.snapshot(&environment.id).await?;
            let code_hash = match payload.code_hash {
                Some(code_hash) if !code_hash.trim().is_empty() => normalize_hash(&code_hash)?,
                _ => {
                    let address = payload.address.as_deref().ok_or_else(|| {
                        ContractFacadeError::InvalidRequest(
                            "Provide address or code_hash".to_owned(),
                        )
                    })?;
                    let canonical = canonical_address(address)?;
                    fetch_single_account_state(
                        http_client,
                        environment,
                        toncenter_api_key,
                        &canonical,
                    )
                    .await?
                    .code_hash
                    .as_deref()
                    .map(normalize_hash)
                    .transpose()?
                    .unwrap_or_default()
                }
            };
            let source = snapshot.latest_verified_source(&code_hash).map_or_else(
                || {
                    serde_json::json!({
                        "code_hash": (!code_hash.is_empty()).then_some(code_hash),
                        "verified": false,
                        "bundle": null,
                    })
                },
                |source| source.source.clone(),
            );
            Ok(success(source))
        }
        ContractRoute::ListVerifiedSources => {
            let snapshot = store.snapshot(&environment.id).await?;
            let mut sources = snapshot
                .verified_sources
                .values()
                .cloned()
                .collect::<Vec<_>>();
            sources.sort_by(|left, right| {
                right
                    .saved_at
                    .cmp(&left.saved_at)
                    .then_with(|| left.artifact_id.cmp(&right.artifact_id))
            });
            Ok(success(sources))
        }
        ContractRoute::RegisterVerifiedSources => {
            let payload: RegisterVerifiedSourcesRequest = json_body(request).await?;
            store
                .register_verified_sources(&environment.id, &payload.entries)
                .await?;
            Ok(success(Value::Null))
        }
        ContractRoute::DeleteVerifiedSource => {
            let payload: CodeHashRequest = json_body(request).await?;
            store
                .delete_verified_source(&environment.id, &payload.code_hash)
                .await?;
            Ok(success(Value::Null))
        }
        ContractRoute::DeleteVerifiedSourceArtifact => {
            let payload: ArtifactIdRequest = json_body(request).await?;
            store
                .delete_verified_source_artifact(&environment.id, &payload.artifact_id)
                .await?;
            Ok(success(Value::Null))
        }
    }
}

async fn list_contracts(
    store: &ContractRegistryStore,
    http_client: &reqwest::Client,
    environment: &StudioEnvironment,
    toncenter_api_key: Option<&HeaderValue>,
) -> Result<Vec<ContractListEntry>, ContractFacadeError> {
    let mut snapshot = store.snapshot(&environment.id).await?;
    let addresses = snapshot
        .contracts
        .keys()
        .chain(snapshot.deployment_candidates.keys())
        .cloned()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    if addresses.is_empty() {
        return Ok(Vec::new());
    }

    let mut states =
        fetch_registered_account_states(http_client, environment, toncenter_api_key, &addresses)
            .await?;
    let confirmed = states
        .iter()
        .filter_map(|state| {
            let canonical = canonical_address(&state.address).ok()?;
            let candidate = snapshot.deployment_candidates.get(&canonical)?;
            let code_hash = state
                .code_hash
                .as_deref()
                .and_then(|value| normalize_hash(value).ok())?;
            (state.status == "active"
                && code_hash == candidate.code_hash
                && snapshot.latest_verified_source(&code_hash).is_some())
            .then_some(canonical)
        })
        .collect::<Vec<_>>();
    if !confirmed.is_empty() {
        store
            .confirm_deployment_candidates(&environment.id, &confirmed)
            .await?;
        snapshot = store.snapshot(&environment.id).await?;
    }
    states.sort_by(|left, right| {
        account_transaction_lt(right)
            .cmp(&account_transaction_lt(left))
            .then_with(|| left.address.cmp(&right.address))
    });
    let mut contracts = Vec::with_capacity(states.len());
    for state in states {
        if state.status != "active" || state.code_hash.is_none() {
            continue;
        }
        let canonical = canonical_address(&state.address)?;
        if !snapshot.contracts.contains_key(&canonical) {
            continue;
        }
        let mut contract =
            contract_from_account_state(&state, &snapshot, registered_source_kind(environment))?;
        enrich_contract(&mut contract, &snapshot);
        contracts.push(contract);
    }
    Ok(contracts)
}

async fn fetch_single_account_state(
    http_client: &reqwest::Client,
    environment: &StudioEnvironment,
    toncenter_api_key: Option<&HeaderValue>,
    requested_canonical_address: &str,
) -> Result<AccountStateFull, ContractFacadeError> {
    fetch_account_states(
        http_client,
        environment,
        toncenter_api_key,
        &[requested_canonical_address.to_owned()],
    )
    .await?
    .into_iter()
    .find(|state| {
        canonical_address(&state.address)
            .is_ok_and(|address| address == requested_canonical_address)
    })
    .ok_or_else(|| {
        ContractFacadeError::InvalidRequest(format!(
            "Account {} is not an active deployed contract",
            display_address(requested_canonical_address)
                .unwrap_or_else(|_| requested_canonical_address.to_owned())
        ))
    })
}

async fn fetch_registered_account_states(
    http_client: &reqwest::Client,
    environment: &StudioEnvironment,
    toncenter_api_key: Option<&HeaderValue>,
    canonical_addresses: &[String],
) -> Result<Vec<AccountStateFull>, ContractFacadeError> {
    let mut states = Vec::new();
    for addresses in canonical_addresses.chunks(MAX_ACCOUNT_STATES_BATCH_SIZE) {
        states.extend(
            fetch_account_states(http_client, environment, toncenter_api_key, addresses).await?,
        );
    }
    Ok(states)
}

async fn fetch_account_states(
    http_client: &reqwest::Client,
    environment: &StudioEnvironment,
    toncenter_api_key: Option<&HeaderValue>,
    canonical_addresses: &[String],
) -> Result<Vec<AccountStateFull>, ContractFacadeError> {
    let api_v3 = environment
        .runtime_endpoints
        .api_v3
        .as_deref()
        .ok_or_else(|| ContractFacadeError::Unavailable("V3 API is unavailable".to_owned()))?;
    let mut url = reqwest::Url::parse(&format!("{}/accountStates", api_v3.trim_end_matches('/')))
        .map_err(|error| {
        ContractFacadeError::Unavailable(format!("V3 API endpoint is invalid: {error}"))
    })?;
    {
        let mut query = url.query_pairs_mut();
        for address in canonical_addresses {
            query.append_pair("address", address);
        }
        query.append_pair("include_boc", "false");
    }
    let response =
        apply_environment_upstream_auth(http_client.get(url), environment, toncenter_api_key)
            .send()
            .await
            .map_err(upstream_error)?;
    let status = response.status();
    let body = response.text().await.map_err(upstream_error)?;
    if !status.is_success() {
        return Err(ContractFacadeError::Upstream(upstream_message(
            status, &body,
        )));
    }
    serde_json::from_str::<AccountStatesResponse>(&body)
        .map(|response| response.accounts)
        .map_err(|error| {
            ContractFacadeError::Upstream(format!(
                "V3 API returned invalid account states: {error}"
            ))
        })
}

fn ensure_active_contract(
    state: &AccountStateFull,
    requested_address: &str,
) -> Result<(), ContractFacadeError> {
    if state.status == "active" && state.code_hash.is_some() {
        return Ok(());
    }
    let display = canonical_address(requested_address)
        .and_then(|address| display_address(&address))
        .unwrap_or_else(|_| requested_address.to_owned());
    Err(ContractFacadeError::InvalidRequest(format!(
        "Account {display} is not an active deployed contract"
    )))
}

fn contract_from_account_state(
    state: &AccountStateFull,
    snapshot: &RegistrySnapshot,
    source_kind: ContractSourceKind,
) -> Result<ContractListEntry, ContractFacadeError> {
    let canonical = canonical_address(&state.address)?;
    let code_hash = state
        .code_hash
        .as_deref()
        .map(normalize_hash)
        .transpose()?
        .unwrap_or_default();
    Ok(ContractListEntry {
        address: display_address(&canonical)?,
        status: contract_status(&state.status),
        code_hash,
        name: snapshot.address_name(&canonical).map(ToOwned::to_owned),
        abi_name: None,
        source_kind,
        artifact: None,
    })
}

fn enrich_contract(contract: &mut ContractListEntry, snapshot: &RegistrySnapshot) {
    if let Ok(address) = canonical_address(&contract.address)
        && let Some(name) = snapshot.address_name(&address)
    {
        contract.name = Some(name.to_owned());
    }
    if contract.code_hash.is_empty() {
        return;
    }
    if let Some(abi) = snapshot.compiler_abi(&contract.code_hash) {
        contract.abi_name = compiler_abi_name(&abi.abi).or_else(|| contract.abi_name.take());
    }
    if let Some(source) = snapshot.latest_verified_source(&contract.code_hash) {
        contract.artifact = Some(contract_artifact(source));
    }
}

fn contract_artifact(source: &SavedVerifiedSource) -> ContractArtifact {
    ContractArtifact {
        artifact_id: source.artifact_id.clone(),
        entrypoint: source
            .source
            .pointer("/bundle/entrypoint")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned),
        compiler_language: source
            .source
            .pointer("/bundle/compiler/language")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned),
        compiler_version: source
            .source
            .pointer("/bundle/compiler/version")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned),
    }
}

fn compiler_abi_name(abi: &Value) -> Option<String> {
    abi.get("compiler_abi")
        .unwrap_or(abi)
        .get("contract_name")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .map(ToOwned::to_owned)
}

const fn registered_source_kind(environment: &StudioEnvironment) -> ContractSourceKind {
    match &environment.config {
        EnvironmentConfig::FullTonNetwork { .. } | EnvironmentConfig::RemoteTonNetwork { .. } => {
            ContractSourceKind::Network
        }
        EnvironmentConfig::ActonLocalnet {
            fork_network: Some(_),
            ..
        } => ContractSourceKind::Fork,
        EnvironmentConfig::ActonLocalnet { .. } => ContractSourceKind::Local,
    }
}

fn contract_status(status: &str) -> String {
    match status {
        "uninit" => "uninitialized",
        value => value,
    }
    .to_owned()
}

fn account_transaction_lt(account: &AccountStateFull) -> u128 {
    account
        .last_transaction_lt
        .as_deref()
        .and_then(|value| value.parse().ok())
        .unwrap_or_default()
}

fn canonical_address(value: &str) -> Result<String, ContractFacadeError> {
    TonAddress::from_str(value.trim())
        .map(|address| address.to_hex())
        .map_err(|_| ContractFacadeError::InvalidRequest(format!("Invalid TON address {value}")))
}

fn display_address(canonical_address: &str) -> Result<String, ContractFacadeError> {
    TonAddress::from_str(canonical_address)
        .map(|address| address.to_base64(false, true, true))
        .map_err(|_| {
            ContractFacadeError::InvalidRequest(format!("Invalid TON address {canonical_address}"))
        })
}

fn normalize_hash(value: &str) -> Result<String, ContractFacadeError> {
    TonHash::from_str(value.trim())
        .map(|hash| hash.to_hex())
        .map_err(|_| ContractFacadeError::InvalidRequest("Invalid TON hash".to_owned()))
}

fn route(method: &Method, path: &str) -> Option<ContractRoute> {
    contract_route(path).filter(|route| route.accepts(method))
}

fn contract_route(path: &str) -> Option<ContractRoute> {
    match path.trim_matches('/') {
        "acton_getAddressName" => Some(ContractRoute::GetAddressName),
        "acton_setAddressName" => Some(ContractRoute::SetAddressName),
        "acton_listContracts" => Some(ContractRoute::ListContracts),
        "acton_registerContract" => Some(ContractRoute::RegisterContract),
        "acton_deleteContract" => Some(ContractRoute::DeleteContract),
        "acton_getCompilerAbi" => Some(ContractRoute::GetCompilerAbi),
        "acton_listCompilerAbis" => Some(ContractRoute::ListCompilerAbis),
        "acton_registerCompilerAbis" => Some(ContractRoute::RegisterCompilerAbis),
        "acton_deleteCompilerAbi" => Some(ContractRoute::DeleteCompilerAbi),
        "acton_getRegisteredVerifiedSource" => Some(ContractRoute::GetRegisteredVerifiedSource),
        "acton_listVerifiedSources" => Some(ContractRoute::ListVerifiedSources),
        "acton_registerVerifiedSources" => Some(ContractRoute::RegisterVerifiedSources),
        "acton_deleteVerifiedSource" => Some(ContractRoute::DeleteVerifiedSource),
        "acton_deleteVerifiedSourceArtifact" => Some(ContractRoute::DeleteVerifiedSourceArtifact),
        _ => None,
    }
}

async fn json_body<T: DeserializeOwned>(request: Request) -> Result<T, ContractFacadeError> {
    let bytes = to_bytes(request.into_body(), MAX_EXTENSION_BODY_BYTES)
        .await
        .map_err(|error| {
            ContractFacadeError::InvalidRequest(format!("Failed to read request body: {error}"))
        })?;
    serde_json::from_slice(&bytes)
        .map_err(|error| ContractFacadeError::InvalidRequest(format!("Invalid JSON: {error}")))
}

fn query<T: DeserializeOwned>(request: &Request) -> Result<T, ContractFacadeError> {
    let value =
        query_pairs(request)
            .into_iter()
            .fold(serde_json::Map::new(), |mut query, (key, value)| {
                query.insert(key, Value::String(value));
                query
            });
    serde_json::from_value(Value::Object(value))
        .map_err(|error| ContractFacadeError::InvalidRequest(format!("Invalid query: {error}")))
}

fn query_pairs(request: &Request) -> Vec<(String, String)> {
    url::form_urlencoded::parse(request.uri().query().unwrap_or_default().as_bytes())
        .map(|(key, value)| (key.into_owned(), value.into_owned()))
        .collect()
}

fn success<T: Serialize>(result: T) -> Response {
    Json(TonlibResponse {
        ok: true,
        result,
        extra: extra(),
    })
    .into_response()
}

fn extra() -> String {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or_else(
        |_| "0".to_owned(),
        |duration| duration.as_millis().to_string(),
    )
}

fn upstream_error(error: reqwest::Error) -> ContractFacadeError {
    ContractFacadeError::Upstream(format!("Failed to reach environment V3 API: {error}"))
}

fn upstream_message(status: StatusCode, body: &str) -> String {
    serde_json::from_str::<Value>(body)
        .ok()
        .and_then(|value| {
            value
                .get("error")
                .and_then(Value::as_str)
                .map(ToOwned::to_owned)
        })
        .unwrap_or_else(|| format!("Environment API request failed with status {status}"))
}

#[derive(Debug, thiserror::Error)]
enum ContractFacadeError {
    #[error("{0}")]
    InvalidRequest(String),
    #[error("{0}")]
    Unavailable(String),
    #[error("{0}")]
    Upstream(String),
    #[error("Method not allowed")]
    MethodNotAllowed,
    #[error(transparent)]
    Registry(#[from] ContractRegistryError),
}

impl IntoResponse for ContractFacadeError {
    fn into_response(self) -> Response {
        let status = match &self {
            Self::InvalidRequest(_) => StatusCode::UNPROCESSABLE_ENTITY,
            Self::Unavailable(_) => StatusCode::CONFLICT,
            Self::Upstream(_) => StatusCode::BAD_GATEWAY,
            Self::MethodNotAllowed => StatusCode::METHOD_NOT_ALLOWED,
            Self::Registry(ContractRegistryError::InvalidRegistration { .. }) => {
                StatusCode::UNPROCESSABLE_ENTITY
            }
            Self::Registry(_) => StatusCode::INTERNAL_SERVER_ERROR,
        };
        (
            status,
            Json(TonlibErrorResponse {
                ok: false,
                error: self.to_string(),
                code: i32::from(status.as_u16()),
                extra: extra(),
                jsonrpc: None,
                id: None,
            }),
        )
            .into_response()
    }
}
