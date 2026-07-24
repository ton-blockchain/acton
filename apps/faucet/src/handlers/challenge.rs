use axum::{
    Extension, Json,
    extract::State,
    http::{HeaderMap, StatusCode},
};
use faucet_backend::middlewares::ClientContext;
use faucet_valkey::{AntifraudModule, CappedEphemeralStoreDecision};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::AppState;
use crate::github_auth::FaucetTier;
use crate::handlers::address::{AddressValidationError, parse_testnet_address};
use crate::handlers::auth;

// The shared hash tag keeps the index and challenge values in one Redis Cluster slot.
const POW_CHALLENGE_KEY_PREFIX: &str = "faucet:pow:{challenges}:challenge";
pub(super) const POW_CHALLENGE_INDEX_KEY: &str = "faucet:pow:{challenges}:active";

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

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub(super) struct ChallengeContext {
    pub(super) version: u32,
    pub(super) address: String,
    pub(super) device_uid: String,
    pub(super) github_user_id: Option<u64>,
    pub(super) tier: FaucetTier,
    pub(super) max_requests: u32,
}

impl ChallengeContext {
    pub(super) fn matches_claim(
        &self,
        address: &str,
        device_uid: &str,
        github_user_id: Option<u64>,
        tier: FaucetTier,
        max_requests: u32,
    ) -> bool {
        self.address == address
            && self.device_uid == device_uid
            && self.github_user_id == github_user_id
            && self.tier == tier
            && self.max_requests == max_requests
    }
}

type ChallengeResult =
    Result<(StatusCode, Json<ChallengeResponse>), (StatusCode, Json<ErrorResponse>)>;

pub(super) async fn create_challenge(
    State(state): State<AppState>,
    Extension(client): Extension<ClientContext>,
    headers: HeaderMap,
    Json(payload): Json<ChallengeRequest>,
) -> ChallengeResult {
    let address = match parse_testnet_address(&payload.address) {
        Ok(address) => address.to_hex(),
        Err(AddressValidationError::Invalid) => {
            return Err(bad_request("Invalid TON address"));
        }
        Err(AddressValidationError::Mainnet) => {
            return Err(bad_request("Testnet TON address required"));
        }
    };

    if payload.token_type == 0 {
        return Err(bad_request("Invalid challenge type"));
    }

    let identity = auth::optional_identity(&state, &headers, &client)
        .await
        .map_err(|(status, _)| response_error(status, "Invalid or expired GitHub session"))?;
    let max_requests = auth::effective_max_requests(&state, identity.as_ref());

    if state.antifraud.wallet_balance_enabled() {
        let balance = state
            .client
            .get_address_balance(&address)
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
    let context = ChallengeContext {
        version,
        address,
        device_uid: client.device_uid,
        github_user_id: identity.as_ref().map(|identity| identity.github_user_id),
        tier: identity
            .as_ref()
            .map(|identity| identity.tier)
            .unwrap_or(FaucetTier::Guest),
        max_requests,
    };
    let encoded_context = serde_json::to_string(&context).map_err(|_| {
        response_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Failed to create challenge",
        )
    })?;
    let store_decision = state
        .valkey
        .store_capped_ephemeral(
            POW_CHALLENGE_INDEX_KEY,
            &challenge_key(&challenge),
            &encoded_context,
            state.config.pow.challenge_ttl_seconds,
            state.config.pow.max_challenges,
        )
        .await
        .map_err(|_| {
            response_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to create challenge",
            )
        })?;
    if store_decision == CappedEphemeralStoreDecision::Full {
        return Err(response_error(
            StatusCode::TOO_MANY_REQUESTS,
            "Too many active challenges",
        ));
    }

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

pub(super) fn challenge_key(challenge: &str) -> String {
    format!(
        "{POW_CHALLENGE_KEY_PREFIX}:{}",
        hex::encode(Sha256::digest(challenge.as_bytes()))
    )
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

    use crate::github_auth::FaucetTier;

    use super::{ChallengeContext, ChallengeRequest, ChallengeResponse, challenge_key};

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

    #[test]
    fn hashes_pow_challenge_before_using_it_as_a_valkey_key() {
        let key = challenge_key("challenge");

        assert_eq!(
            key,
            "faucet:pow:{challenges}:challenge:2dd00bd77e0222ced882665481a9c1d9f907309d16e05ed007a1ea63928477a9"
        );
        assert!(!key.ends_with(":challenge"));
    }

    #[test]
    fn binds_challenge_to_address_browser_and_github_session() {
        let context = ChallengeContext {
            version: 1,
            address: "0:abc".to_string(),
            device_uid: "12345678-1234-1234-1234-123456789abc".to_string(),
            github_user_id: Some(42),
            tier: FaucetTier::Verified,
            max_requests: 4,
        };

        assert!(context.matches_claim(
            "0:abc",
            "12345678-1234-1234-1234-123456789abc",
            Some(42),
            FaucetTier::Verified,
            4,
        ));
        assert!(!context.matches_claim(
            "0:def",
            "12345678-1234-1234-1234-123456789abc",
            Some(42),
            FaucetTier::Verified,
            4,
        ));
        assert!(!context.matches_claim(
            "0:abc",
            "abcdefab-abcd-abcd-abcd-abcdefabcdef",
            Some(42),
            FaucetTier::Verified,
            4,
        ));
        assert!(!context.matches_claim(
            "0:abc",
            "12345678-1234-1234-1234-123456789abc",
            Some(43),
            FaucetTier::Verified,
            4,
        ));
    }
}
