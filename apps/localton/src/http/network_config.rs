//! Applies on-chain configuration through the bootstrap config contract's master key.
//!
//! The key stays inside Localton. Requests use compare-and-set semantics and only
//! succeed once the masterchain exposes the requested cell, rather than merely
//! accepting an external message into the config contract.

use std::{
    sync::Arc,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use anyhow::{Context, Result, anyhow, ensure};
use axum::{Json, extract::State as AxumState};
use base64::{Engine, engine::general_purpose::STANDARD};
use ed25519_dalek::{Signer, SigningKey};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tokio::sync::Mutex;
use tracing::info;
use tycho_types::{
    boc::Boc,
    cell::{Cell, CellBuilder},
    dict::Dict,
};
use utoipa::ToSchema;

use super::error::HttpError;
use crate::storage::Layout;

#[cfg(test)]
mod tests;

#[derive(Clone)]
pub(super) struct State {
    layout: Layout,
    backend: String,
    client: reqwest::Client,
    lock: Arc<Mutex<()>>,
}

impl State {
    pub(super) fn new(layout: Layout, backend: String) -> Self {
        Self {
            layout,
            backend,
            client: reqwest::Client::new(),
            lock: Arc::new(Mutex::new(())),
        }
    }

    async fn query(&self, method: &str, payload: Value) -> Result<Value> {
        let response: Value = self
            .client
            .post(format!("{}/api/v2/{method}", self.backend))
            .json(&payload)
            .timeout(Duration::from_secs(10))
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;
        ensure!(
            response["ok"] == true,
            "{method} failed: {}",
            response["error"]
        );
        response
            .get("result")
            .cloned()
            .context("V2 response has no result")
    }

    async fn config(&self) -> Result<Dict<u32, Cell>> {
        let result = self.query("getConfigAll", json!({})).await?;
        let bytes = result["config"]["bytes"]
            .as_str()
            .context("Config response has no BoC")?;
        Ok(Dict::from_raw(Some(Boc::decode(STANDARD.decode(bytes)?)?)))
    }
}

/// One parameter mutation. A missing expected hash means the parameter must not exist.
#[derive(Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct UpdateRequest {
    index: i32,
    boc: String,
    expected_hash: Option<String>,
}

/// Evidence that a requested parameter became visible in the masterchain config.
#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(super) struct UpdateResult {
    index: i32,
    hash: String,
    masterchain_seqno: u32,
}

/// Update a parameter in the running blockchain and wait for masterchain confirmation
#[utoipa::path(
    post,
    path = "/v1/network/config",
    request_body = UpdateRequest,
    responses(
        (status = 200, description = "Parameter confirmed in masterchain", body = UpdateResult),
        (
            status = 400,
            description = "Invalid, stale or unconfirmed update",
            body = super::error::ErrorResponse
        )
    ),
    tag = "administration"
)]
pub(super) async fn update_handler(
    AxumState(state): AxumState<State>,
    Json(request): Json<UpdateRequest>,
) -> Result<Json<UpdateResult>, HttpError> {
    let _guard = state
        .lock
        .try_lock()
        .context("Another config update is in progress")?;
    let started = Instant::now();
    info!(
        operation = "update_config",
        target = request.index,
        stage = "validating",
        "network config update started"
    );

    let result = match tokio::time::timeout(Duration::from_secs(60), apply(&state, &request)).await
    {
        Ok(result) => result,
        Err(_) => Err(anyhow!(
            "Config confirmation timed out; reload the active config before retrying"
        )),
    };

    info!(
        operation = "update_config",
        target = request.index,
        duration_ms = started.elapsed().as_millis(),
        outcome = if result.is_ok() {
            "confirmed"
        } else {
            "failed"
        },
        "network config update finished"
    );
    Ok(Json(result?))
}

async fn apply(state: &State, request: &UpdateRequest) -> Result<UpdateResult> {
    ensure!(
        request.index != 0,
        "Parameter 0 identifies this config contract and cannot be changed in place"
    );
    ensure!(
        request.boc.len() <= 1_000_000,
        "Parameter BoC exceeds the size limit"
    );
    let cell = Boc::decode(
        STANDARD
            .decode(&request.boc)
            .context("Parameter must be a base64 BoC")?,
    )?;
    ensure!(
        !cell.is_exotic() && cell.repr_depth() < 120,
        "Parameter must be an ordinary cell with depth below 120"
    );
    let config = state.config().await?;
    let previous = config.get(request.index as u32)?;
    ensure!(
        previous.as_ref().map(|value| value.repr_hash().to_string()) == request.expected_hash,
        "Parameter changed since it was loaded; reload the configuration before applying"
    );
    ensure!(
        previous
            .as_ref()
            .is_none_or(|value| value.repr_hash() != cell.repr_hash()),
        "The parameter has not changed"
    );

    let config_address = config
        .get(0)?
        .context("Network has no configuration contract")?;
    let address_hash = config_address.as_slice()?.load_u256()?;
    let address = format!("-1:{address_hash}");
    let account = state
        .query("getAddressInformation", json!({"address": address}))
        .await?;
    let data = Boc::decode(
        STANDARD.decode(
            account["data"]
                .as_str()
                .context("Config account has no data")?,
        )?,
    )?;
    let mut data = data.as_slice()?;
    let contract_config = Dict::<u32, Cell>::from_raw(Some(data.load_reference_cloned()?));
    let seqno = data.load_u32()?;
    let public_key = data.load_u256()?;
    ensure!(
        contract_config
            .get(request.index as u32)?
            .as_ref()
            .map(|value| value.repr_hash())
            == previous.as_ref().map(|value| value.repr_hash()),
        "Config contract has an unapplied change for this parameter; inspect its state before retrying"
    );

    let key_path = state.layout.zerostate.join("config-master.pk");
    let key: [u8; 32] = tokio::fs::read(&key_path)
        .await
        .with_context(|| {
            format!(
                "Cannot read configuration master key at {}",
                key_path.display()
            )
        })?
        .try_into()
        .map_err(|_| anyhow::anyhow!("Invalid configuration master key length"))?;
    let key = SigningKey::from_bytes(&key);
    ensure!(
        key.verifying_key().as_bytes() == public_key.as_array(),
        "This Localton instance does not own the configuration master key"
    );

    let valid_until = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs() as u32 + 45;
    let message = signed_update(
        &key,
        address_hash.as_array(),
        seqno,
        valid_until,
        request.index,
        cell.clone(),
    )?;
    info!(
        operation = "update_config",
        target = request.index,
        stage = "submitting",
        "submitting configuration parameter"
    );
    state
        .query(
            "sendBocReturnHash",
            json!({"boc": STANDARD.encode(Boc::encode(message))}),
        )
        .await?;

    // Contract acceptance alone is insufficient: validators may reject a malformed
    // dictionary. Observe the active masterchain config before reporting success.
    loop {
        tokio::time::sleep(Duration::from_millis(500)).await;
        let config = state.config().await?;
        if config
            .get(request.index as u32)?
            .is_some_and(|value| value.repr_hash() == cell.repr_hash())
        {
            let head = state.query("getMasterchainInfo", json!({})).await?;
            return Ok(UpdateResult {
                index: request.index,
                hash: cell.repr_hash().to_string(),
                masterchain_seqno: head["last"]["seqno"]
                    .as_u64()
                    .context("Missing masterchain sequence number")?
                    as u32,
            });
        }
    }
}

/// Encodes the official config contract's signed `change one parameter` action.
/// The expiry and contract sequence number prevent replay across later updates.
fn signed_update(
    key: &SigningKey,
    address: &[u8; 32],
    seqno: u32,
    valid_until: u32,
    index: i32,
    value: Cell,
) -> Result<Cell> {
    let mut unsigned = CellBuilder::new();
    unsigned.store_u32(0x4366_5021)?;
    unsigned.store_u32(seqno)?;
    unsigned.store_u32(valid_until)?;
    unsigned.store_u32(index as u32)?;
    unsigned.store_reference(value)?;
    let unsigned = unsigned.build()?;
    let signature = key.sign(unsigned.repr_hash().as_slice());
    let mut body = CellBuilder::new();
    body.store_raw(&signature.to_bytes(), 512)?;
    body.store_slice(unsigned.as_slice()?)?;

    let mut message = CellBuilder::new();
    message.store_small_uint(0b100010, 6)?; // external inbound, no source, standard destination
    message.store_bit_zero()?; // no anycast
    message.store_u8(255)?; // masterchain (-1)
    message.store_raw(address, 256)?;
    message.store_small_uint(0, 4)?; // zero import fee
    message.store_bit_zero()?; // no state init
    message.store_bit_one()?; // body by reference
    message.store_reference(body.build()?)?;
    Ok(message.build()?)
}
