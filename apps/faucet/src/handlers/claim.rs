use crate::AppState;
use apalis::prelude::TaskSink;
use axum::{Json, extract::State, http::StatusCode};
use faucet_valkey::{AntifraudModule, SuccessfulClaimWindowDecision};
use serde::{Deserialize, Serialize};
use std::str::FromStr;
use ton::ton_core::types::TonAddress;
use tracing::{error, info, warn};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub(crate) struct CreateClaim {
    pub(crate) address: String,
    pub(crate) challenge: String,
    pub(crate) nonce: u64,
}

#[derive(Deserialize)]
pub(super) struct CreateClaimRequest {
    address: String,
    challenge: String,
    nonce: u64,
    version: u32,
}

#[derive(Serialize)]
pub(super) struct ClaimResponse {
    message: &'static str,
}

#[derive(Serialize)]
pub(super) struct ErrorResponse {
    error: &'static str,
}

type ClaimResult = Result<(StatusCode, Json<ClaimResponse>), (StatusCode, Json<ErrorResponse>)>;
type ClaimLimitResult = Result<(), (StatusCode, Json<ErrorResponse>)>;

//noinspection RsLiveness
#[axum::debug_handler]
pub(super) async fn create_claim(
    State(mut state): State<AppState>,
    Json(payload): Json<CreateClaimRequest>,
) -> ClaimResult {
    let address = TonAddress::from_str(&payload.address)
        .map(|address| address.to_hex())
        .map_err(|_| bad_request("Invalid TON address"))?;

    if !state.pow.can_process_version(payload.version) {
        return Err(bad_request("Unsupported challenge version"));
    }

    let challenge_version = state
        .pow_challenges
        .get(&payload.challenge)
        .ok_or_else(|| bad_request("Invalid or expired challenge"))?;

    if challenge_version != payload.version {
        return Err(bad_request("Invalid challenge version"));
    }

    if !state.pow.verify(&payload.challenge, payload.nonce) {
        return Err(bad_request("Invalid PoW solution"));
    }

    check_successful_claim_window(&state, &address).await?;

    if state.pow_challenges.remove(&payload.challenge).is_none() {
        return Err(bad_request("Invalid or expired challenge"));
    }

    state
        .storage
        .push(CreateClaim {
            address,
            challenge: payload.challenge,
            nonce: payload.nonce,
        })
        .await
        .map_err(|_| response_error(StatusCode::INTERNAL_SERVER_ERROR, "Failed to queue claim"))?;

    Ok((
        StatusCode::OK,
        Json(ClaimResponse {
            message: "Your claim has been queued. It will be processed soon.",
        }),
    ))
}

// TODO: сделать по другому
async fn check_successful_claim_window(state: &AppState, address: &str) -> ClaimLimitResult {
    let Some(window) = state.antifraud.successful_claim_window() else {
        return Ok(());
    };

    match state
        .valkey
        .check_successful_claim_window(address, window.max_requests, window.window_seconds)
        .await
    {
        Ok(SuccessfulClaimWindowDecision::Allowed {
            current,
            max,
            window_seconds,
        }) => {
            info!(
                address = %address,
                successful_claims = current,
                max_requests = max,
                window_seconds,
                "Successful claim window checked"
            );
            Ok(())
        }
        Ok(SuccessfulClaimWindowDecision::Limited {
            current,
            max,
            window_seconds,
            retry_after_ms,
        }) => {
            state
                .record_antifraud_trigger(AntifraudModule::SuccessfulClaimWindow)
                .await;
            warn!(
                address = %address,
                successful_claims = current,
                max_requests = max,
                window_seconds,
                retry_after_ms,
                "Successful claim window limit reached"
            );
            Err(response_error(
                StatusCode::TOO_MANY_REQUESTS,
                "Successful claim limit exceeded",
            ))
        }
        Err(err) => {
            error!(
                address = %address,
                error = %err,
                "Failed to check successful claim window"
            );
            Err(response_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to check claim limit",
            ))
        }
    }
}

fn bad_request(error: &'static str) -> (StatusCode, Json<ErrorResponse>) {
    response_error(StatusCode::BAD_REQUEST, error)
}

fn response_error(status: StatusCode, error: &'static str) -> (StatusCode, Json<ErrorResponse>) {
    (status, Json(ErrorResponse { error }))
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::CreateClaimRequest;

    #[test]
    fn deserializes_challenge_version() {
        let request: CreateClaimRequest = serde_json::from_value(json!({
            "address": "UQAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAJKZ",
            "challenge": "challenge",
            "nonce": 42,
            "version": 1,
        }))
        .unwrap();

        assert_eq!(
            request.address,
            "UQAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAJKZ"
        );
        assert_eq!(request.challenge, "challenge");
        assert_eq!(request.nonce, 42);
        assert_eq!(request.version, 1);
    }
}
