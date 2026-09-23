//! Implements `POST /acton_fundAccount` for the admin API.
//!
//! The handler validates the destination and amount, locks the faucet wallet,
//! builds a signed external message, and submits its BoC with
//! `sendBocReturnHash`. It then polls `getTransactions` for the faucet account
//! and returns the hash of the confirmed internal transfer to the destination.

use std::{path::PathBuf, sync::Arc, time::Duration};

use anyhow::{Result, bail};
use axum::{
    Json,
    extract::State as AxumState,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use base64::{Engine, engine::general_purpose::STANDARD};
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;
use toncenter::v2::{
    requests::{SendBocRequest, TransactionsRequest},
    responses::Transaction,
};
use tonutils::tvm::Address;
use utoipa::ToSchema;

use crate::operations::wallets;

#[derive(Clone)]
pub(super) struct State {
    client: toncenter_client::Client,
    state_dir: PathBuf,
    lock: Arc<Mutex<()>>,
}

impl State {
    pub(super) fn new(backend: String, state_dir: PathBuf) -> Result<Self> {
        Ok(Self {
            client: toncenter_client::Client::builder()
                .v2_url(format!("{}/api/v2", backend.trim_end_matches('/')))
                .user_agent(concat!("localton/", env!("CARGO_PKG_VERSION")))
                .request_timeout(Duration::from_secs(10))
                .operation_timeout(Duration::from_secs(10))
                .build()?,
            state_dir,
            lock: Arc::new(Mutex::new(())),
        })
    }
}

#[derive(Deserialize, ToSchema)]
pub(super) struct FundAccountRequest {
    /// TON address that receives the funds
    address: String,
    /// Transfer amount in nanograms
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
    let result = state
        .client
        .call_v2::<toncenter::v2::endpoints::SendBocReturnHash>(&SendBocRequest {
            boc: STANDARD.encode(boc),
        })
        .await?;
    anyhow::ensure!(
        !result.hash.is_empty(),
        "sendBocReturnHash response did not include a message hash"
    );
    Ok(result.hash)
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
                        same_ton_address(&outgoing.destination, &message.destination_address)
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

async fn transactions(state: &State, source_address: &str) -> Result<Vec<Transaction>> {
    let request = TransactionsRequest {
        address: source_address.to_owned(),
        limit: Some(TRANSACTION_LOOKBACK.into()),
        lt: None,
        hash: None,
        to_lt: None,
        archival: None,
    };
    Ok(state
        .client
        .v2_request(
            toncenter_client::V2Transport::Get,
            "getTransactions",
            &request,
        )
        .await?)
}

fn same_ton_address(left: &str, right: &str) -> bool {
    match (Address::from_str(left), Address::from_str(right)) {
        (Ok(left), Ok(right)) => left == right,
        _ => left == right,
    }
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
