//! Implements `POST /acton_fundAccount` for the admin API.
//!
//! The handler validates the destination and amount, locks the faucet wallet,
//! builds a signed external message, and submits its BoC with
//! `sendBocReturnHash`. It then polls `getTransactions` for the faucet account
//! and returns the hash of the confirmed internal transfer to the destination.

use std::{path::PathBuf, sync::Arc, time::Duration};

use anyhow::{Context, Result, bail};
use axum::{
    Json,
    extract::State as AxumState,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use base64::{Engine, engine::general_purpose::STANDARD};
use serde::{Deserialize, Serialize};
use serde_json::json;
use tokio::sync::Mutex;
use tonutils::tvm::Address;
use utoipa::ToSchema;

use crate::operations::wallets;

#[derive(Clone)]
pub(super) struct State {
    backend: String,
    client: reqwest::Client,
    state_dir: PathBuf,
    lock: Arc<Mutex<()>>,
}

impl State {
    pub(super) fn new(backend: String, state_dir: PathBuf) -> Self {
        Self {
            backend,
            client: reqwest::Client::new(),
            state_dir,
            lock: Arc::new(Mutex::new(())),
        }
    }
}

#[derive(Deserialize, ToSchema)]
pub(super) struct FundAccountRequest {
    /// TON address that receives the funds
    address: String,
    /// Transfer amount in nanotons
    amount: u128,
}

#[derive(Debug, Serialize, ToSchema)]
pub(super) struct FundAccountResponse {
    /// `true` when Localton confirms the transfer
    ok: bool,
    #[schema(inline)]
    result: FundAccountResult,
}

#[derive(Debug, Serialize, ToSchema)]
struct FundAccountResult {
    /// TON Center response type
    #[serde(rename = "@type")]
    kind: String,
    /// Base64 hash of the confirmed internal message
    hash: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub(super) struct FundAccountErrorResponse {
    /// Always `false` for an error response
    ok: bool,
    /// Error message for the request
    error: String,
    /// HTTP status code for the error
    code: u16,
}

#[derive(Deserialize)]
struct TonlibResponse<T> {
    ok: bool,
    result: Option<T>,
    error: Option<String>,
}

#[derive(Deserialize)]
struct SendBocResult {
    hash: String,
}

#[derive(Deserialize)]
struct RawTransaction {
    in_msg: Option<RawMessageHash>,
    #[serde(default)]
    out_msgs: Vec<RawOutgoingMessage>,
}

#[derive(Deserialize)]
struct RawMessageHash {
    hash: String,
}

#[derive(Deserialize)]
struct RawOutgoingMessage {
    hash: String,
    destination: Option<RawAccountAddress>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum RawAccountAddress {
    Text(String),
    Tonlib { account_address: String },
}

impl RawAccountAddress {
    fn as_str(&self) -> &str {
        match self {
            Self::Text(address) => address,
            Self::Tonlib { account_address } => account_address,
        }
    }
}

const CONFIRMATION_TIMEOUT: Duration = Duration::from_secs(30);
const CONFIRMATION_INTERVAL: Duration = Duration::from_millis(500);
pub(super) const TRANSACTION_LOOKBACK: &str = "16";

/// Fund an account from the genesis wallet
///
/// Localton sends a signed transfer and waits for the destination message
#[utoipa::path(
    post,
    path = "/acton_fundAccount",
    tag = "administration",
    request_body = FundAccountRequest,
    responses(
        (status = 200, description = "Confirmed internal transfer hash", body = FundAccountResponse),
        (status = 400, description = "Invalid address or amount", body = FundAccountErrorResponse),
        (status = 500, description = "Funding message could not be submitted or confirmed", body = FundAccountErrorResponse)
    )
)]
pub(super) async fn fund_account_handler(
    AxumState(state): AxumState<State>,
    Json(payload): Json<FundAccountRequest>,
) -> Response {
    let _guard = state.lock.lock().await;
    let message = match wallets::build_fund_account_message(
        &state.state_dir,
        &payload.address,
        payload.amount,
    )
    .await
    {
        Ok(message) => message,
        Err(wallets::FundAccountError::InvalidRequest(error)) => {
            return fund_account_error(StatusCode::BAD_REQUEST, error);
        }
        Err(wallets::FundAccountError::Infrastructure(error)) => {
            return fund_account_error(StatusCode::INTERNAL_SERVER_ERROR, format!("{error:#}"));
        }
    };

    let result = async {
        let external_hash = send_boc_return_hash(&state, &message.boc).await?;
        wait_for_transfer(&state, &message, &external_hash).await
    }
    .await;
    match result {
        Ok(hash) => Json(FundAccountResponse {
            ok: true,
            result: FundAccountResult {
                kind: "ok".to_owned(),
                hash,
            },
        })
        .into_response(),
        Err(error) => fund_account_error(StatusCode::INTERNAL_SERVER_ERROR, format!("{error:#}")),
    }
}

pub(super) async fn send_boc_return_hash(state: &State, boc: &[u8]) -> Result<String> {
    let url = format!(
        "{}/api/v2/sendBocReturnHash",
        state.backend.trim_end_matches('/')
    );
    let payload = serde_json::to_vec(&json!({ "boc": STANDARD.encode(boc) }))?;
    let response = state
        .client
        .post(url)
        .header("content-type", "application/json")
        .body(payload)
        .timeout(Duration::from_secs(10))
        .send()
        .await
        .context("TON HTTP API sendBocReturnHash request failed")?;
    let status = response.status();
    let body = response
        .bytes()
        .await
        .context("failed to read sendBocReturnHash response")?;
    let response: TonlibResponse<SendBocResult> = serde_json::from_slice(&body)
        .with_context(|| format!("invalid sendBocReturnHash response: {}", body_text(&body)))?;
    if !status.is_success() || !response.ok {
        bail!(
            "sendBocReturnHash failed with status {status}: {}",
            response
                .error
                .unwrap_or_else(|| "TON HTTP API returned no error details".to_owned())
        );
    }
    response
        .result
        .map(|result| result.hash)
        .filter(|hash| !hash.is_empty())
        .context("sendBocReturnHash response did not include a message hash")
}

pub(super) async fn wait_for_transfer(
    state: &State,
    message: &wallets::FundAccountMessage,
    external_hash: &str,
) -> Result<String> {
    let deadline = tokio::time::Instant::now() + CONFIRMATION_TIMEOUT;
    let mut last_error = None;
    loop {
        match transactions(state, &message.source_address).await {
            Ok(transactions) => {
                if let Some(transaction) = transactions.iter().find(|transaction| {
                    transaction
                        .in_msg
                        .as_ref()
                        .is_some_and(|incoming| incoming.hash == external_hash)
                }) {
                    if let Some(outgoing) = transaction.out_msgs.iter().find(|outgoing| {
                        outgoing.destination.as_ref().is_some_and(|destination| {
                            same_ton_address(destination.as_str(), &message.destination_address)
                        })
                    }) {
                        return Ok(outgoing.hash.clone());
                    }
                    bail!(
                        "confirmed faucet message {external_hash} produced no transfer to {}",
                        message.destination_address
                    );
                }
            }
            Err(error) => last_error = Some(format!("{error:#}")),
        }

        if tokio::time::Instant::now() >= deadline {
            let detail = last_error
                .map(|error| format!(": {error}"))
                .unwrap_or_default();
            bail!(
                "faucet message {external_hash} at seqno {} was not confirmed within {} seconds{detail}",
                message.seqno,
                CONFIRMATION_TIMEOUT.as_secs()
            );
        }
        tokio::time::sleep(CONFIRMATION_INTERVAL).await;
    }
}

async fn transactions(state: &State, source_address: &str) -> Result<Vec<RawTransaction>> {
    let mut url = reqwest::Url::parse(&format!(
        "{}/api/v2/getTransactions",
        state.backend.trim_end_matches('/')
    ))?;
    url.query_pairs_mut()
        .append_pair("address", source_address)
        .append_pair("limit", TRANSACTION_LOOKBACK);
    let response = state
        .client
        .get(url)
        .timeout(Duration::from_secs(5))
        .send()
        .await
        .context("TON HTTP API getTransactions request failed")?;
    let status = response.status();
    let body = response
        .bytes()
        .await
        .context("failed to read getTransactions response")?;
    let response: TonlibResponse<Vec<RawTransaction>> = serde_json::from_slice(&body)
        .with_context(|| format!("invalid getTransactions response: {}", body_text(&body)))?;
    if !status.is_success() || !response.ok {
        bail!(
            "getTransactions failed with status {status}: {}",
            response
                .error
                .unwrap_or_else(|| "TON HTTP API returned no error details".to_owned())
        );
    }
    response
        .result
        .context("getTransactions response did not include a result")
}

fn same_ton_address(left: &str, right: &str) -> bool {
    match (Address::from_str(left), Address::from_str(right)) {
        (Ok(left), Ok(right)) => left == right,
        _ => left == right,
    }
}

fn body_text(body: &[u8]) -> String {
    String::from_utf8_lossy(body).chars().take(512).collect()
}

fn fund_account_error(status: StatusCode, error: String) -> Response {
    (
        status,
        Json(FundAccountErrorResponse {
            ok: false,
            error,
            code: status.as_u16(),
        }),
    )
        .into_response()
}
