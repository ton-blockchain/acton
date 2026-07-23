use axum::{Json, extract::State, http::StatusCode};
use faucet_valkey::AntifraudModule;
use serde::{Deserialize, Serialize};
use std::str::FromStr;
use ton::ton_core::types::TonAddress;

use crate::AppState;

#[derive(Deserialize)]
pub(super) struct ChallengeRequest {
    address: String,
    #[serde(rename = "type")]
    token_type: u32,
}

#[derive(Serialize)]
pub(super) struct ChallengeResponse {
    version: u32,
    challenge: String,
    difficulty: u32,
    max_solve_ttl_seconds: u64,
    max_nonce_attempts: u64,
}

#[derive(Serialize)]
pub(super) struct ErrorResponse {
    error: &'static str,
}

type ChallengeResult =
    Result<(StatusCode, Json<ChallengeResponse>), (StatusCode, Json<ErrorResponse>)>;

pub(super) async fn create_challenge(
    State(state): State<AppState>,
    Json(payload): Json<ChallengeRequest>,
) -> ChallengeResult {
    if TonAddress::from_str(&payload.address).is_err() {
        return Err(bad_request("Invalid TON address"));
    }

    if payload.token_type == 0 {
        return Err(bad_request("Invalid challenge type"));
    }

    if state.antifraud.wallet_balance_enabled() {
        let balance = state
            .client
            .get_address_balance(&payload.address)
            .await
            .map_err(|_| {
                response_error(
                    StatusCode::SERVICE_UNAVAILABLE,
                    "Failed to check wallet balance",
                )
            })?;

        if state.antifraud.check_wallet_balance(balance).is_err() {
            state
                .record_antifraud_trigger(AntifraudModule::WalletBalance)
                .await;
            return Err(response_error(
                StatusCode::FORBIDDEN,
                "Wallet balance exceeds limit",
            ));
        }
    }

    let challenge = state.pow.create();
    let version = state.pow.version();

    state.pow_challenges.insert(challenge.clone(), version);

    Ok((
        StatusCode::OK,
        Json(ChallengeResponse {
            version,
            challenge,
            difficulty: state.pow.difficulty(),
            max_solve_ttl_seconds: state.config.pow.client.max_solve_ttl_seconds,
            max_nonce_attempts: state.config.pow.client.max_nonce_attempts,
        }),
    ))
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

    use super::{ChallengeRequest, ChallengeResponse};

    #[test]
    fn serializes_challenge_response() {
        let response = ChallengeResponse {
            version: 1,
            challenge: "challenge".to_string(),
            difficulty: 21,
            max_solve_ttl_seconds: 300,
            max_nonce_attempts: 1_000_000_000,
        };

        assert_eq!(
            serde_json::to_value(response).unwrap(),
            json!({
                "version": 1,
                "challenge": "challenge",
                "difficulty": 21,
                "max_solve_ttl_seconds": 300,
                "max_nonce_attempts": 1_000_000_000_u64,
            })
        );
    }

    #[test]
    fn deserializes_challenge_request() {
        let request: ChallengeRequest = serde_json::from_value(json!({
            "address": "UQAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAJKZ",
            "type": 1,
        }))
        .unwrap();

        assert_eq!(
            request.address,
            "UQAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAJKZ"
        );
        assert_eq!(request.token_type, 1);
    }
}
