use crate::AppState;
use crate::address::{AddressValidationError, parse_testnet_address};
use crate::antifraud_subject;
use crate::github_auth::FaucetTier;
use crate::handlers::{auth, challenge};
use apalis::prelude::TaskSink;
use axum::{
    Extension, Json,
    extract::State,
    http::{HeaderMap, StatusCode},
};
use faucet::middlewares::ClientContext;
use faucet_valkey::{AmountWindowDecision, AntifraudModule, SuccessfulClaimWindowDecision};
use real::RealIp;
use serde::{Deserialize, Serialize};
use tracing::{error, info, warn};
use utoipa::ToSchema;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub(crate) struct CreateClaim {
    pub(crate) address: String,
    pub(crate) challenge: String,
    pub(crate) nonce: u64,
    #[serde(default)]
    pub(crate) github_user_id: Option<u64>,
    #[serde(default)]
    pub(crate) tier: FaucetTier,
    #[serde(default)]
    pub(crate) max_requests: u32,
    #[serde(default)]
    pub(crate) client_window_subject: Option<String>,
    #[serde(default)]
    pub(crate) device_window_subject: Option<String>,
    #[serde(default)]
    pub(crate) subnet_amount_window_subject: Option<String>,
}

#[derive(Deserialize, ToSchema)]
pub(super) struct CreateClaimRequest {
    address: String,
    challenge: String,
    nonce: u64,
    version: u32,
}

#[derive(Serialize, ToSchema)]
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
#[utoipa::path(
    post,
    path = "/claim",
    params(
        ("Authorization" = Option<String>, Header, description = "Optional GitHub session token as Bearer <token>"),
        ("x-device-uid" = String, Header, description = "Stable client device identifier"),
        ("x-acton-client" = Option<String>, Header, description = "Actonscan client version; required unless User-Agent starts with acton/")
    ),
    request_body = CreateClaimRequest,
    responses(
        (status = 200, description = "Claim accepted for asynchronous processing", body = ClaimResponse),
        (status = 400, description = "Invalid request, challenge, proof-of-work solution, or client headers", body = auth::ErrorResponse),
        (status = 401, description = "Invalid or expired GitHub session", body = auth::ErrorResponse),
        (status = 429, description = "Faucet request or amount limit exceeded", body = auth::ErrorResponse),
        (status = 500, description = "Failed to validate or queue the claim", body = auth::ErrorResponse),
        (status = 503, description = "PoW is disabled or a dependency is unavailable", body = auth::ErrorResponse)
    ),
    tag = "faucet"
)]
pub(super) async fn create_claim(
    State(mut state): State<AppState>,
    Extension(client): Extension<ClientContext>,
    Extension(client_ip): Extension<RealIp>,
    headers: HeaderMap,
    Json(payload): Json<CreateClaimRequest>,
) -> ClaimResult {
    info!(
        address = %payload.address,
        client_ip = %client_ip.ip(),
        device_uid = %client.device_uid,
        "Received faucet claim request"
    );

    let address = match parse_testnet_address(&payload.address) {
        Ok(address) => address.to_hex(),
        Err(AddressValidationError::Invalid) => {
            return Err(bad_request("Invalid TON address"));
        }
        Err(AddressValidationError::Mainnet) => {
            return Err(bad_request("Testnet TON address required"));
        }
    };

    if !state.pow.can_process_version(payload.version) {
        return Err(bad_request("Unsupported challenge version"));
    }

    let challenge_key = challenge::challenge_key(&payload.challenge);
    let encoded_context = state
        .valkey
        .get_ephemeral(&challenge_key)
        .await
        .map_err(|_| {
            response_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to load challenge",
            )
        })?
        .ok_or_else(|| bad_request("Invalid or expired challenge"))?;
    let challenge_context: challenge::ChallengeContext = serde_json::from_str(&encoded_context)
        .map_err(|_| {
            response_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to decode challenge",
            )
        })?;

    if challenge_context.version != payload.version {
        return Err(bad_request("Invalid challenge version"));
    }
    let identity = auth::optional_identity(&state, &headers, &client)
        .await
        .map_err(|(status, _)| response_error(status, "Invalid or expired GitHub session"))?;
    let github_user_id = identity.as_ref().map(|identity| identity.github_user_id);
    let tier = identity
        .as_ref()
        .map(|identity| identity.tier)
        .unwrap_or(FaucetTier::Guest);
    let max_requests = auth::effective_max_requests(&state, identity.as_ref());
    if !challenge_context.matches_claim(
        &address,
        &client.device_uid,
        github_user_id,
        tier,
        max_requests,
    ) {
        return Err(bad_request("Challenge authorization does not match claim"));
    }

    if !state.pow.verify(&payload.challenge, payload.nonce) {
        return Err(bad_request("Invalid PoW solution"));
    }

    check_successful_claim_window(&state, &address, max_requests).await?;
    let client_window_subject = antifraud_subject::client_ip(client_ip.ip());
    let device_window_subject = antifraud_subject::device_uid(&client.device_uid);
    if let Some(github_user_id) = github_user_id {
        check_successful_claim_window(
            &state,
            &antifraud_subject::github(github_user_id),
            max_requests,
        )
        .await?;
    }
    check_successful_claim_window(&state, &device_window_subject, max_requests).await?;
    if tier == FaucetTier::Guest {
        check_successful_claim_window(
            &state,
            &client_window_subject,
            state.config.antifraud.successful_claim_window.max_requests,
        )
        .await?;
    }

    let subnet_amount_window_subject = check_subnet_amount_window(&state, client_ip.ip()).await?;

    let consumed_context = state
        .valkey
        .take_capped_ephemeral(challenge::POW_CHALLENGE_INDEX_KEY, &challenge_key)
        .await
        .map_err(|_| {
            response_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to consume challenge",
            )
        })?;
    if consumed_context.as_deref() != Some(encoded_context.as_str()) {
        return Err(bad_request("Invalid or expired challenge"));
    }

    state
        .storage
        .push(CreateClaim {
            address,
            challenge: payload.challenge,
            nonce: payload.nonce,
            github_user_id,
            tier,
            max_requests,
            client_window_subject: Some(client_window_subject),
            device_window_subject: Some(device_window_subject),
            subnet_amount_window_subject,
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

async fn check_subnet_amount_window(
    state: &AppState,
    client_ip: std::net::IpAddr,
) -> Result<Option<String>, (StatusCode, Json<ErrorResponse>)> {
    let Some(window) = state.antifraud.subnet_amount_window() else {
        return Ok(None);
    };
    let amount = state.config.faucet.amount;
    let subject = antifraud_subject::client_subnet(client_ip, window.ipv4_prefix_length);

    if let Err(err) = state.antifraud.check_subnet_amount_window_transfer(amount) {
        state
            .record_antifraud_trigger(AntifraudModule::SubnetAmountWindow)
            .await;
        error!(
            subject,
            amount,
            max_amount = window.max_amount,
            error = ?err,
            "Claim amount exceeds subnet amount window limit"
        );
        return Err(response_error(
            StatusCode::TOO_MANY_REQUESTS,
            "Subnet amount limit exceeded",
        ));
    }

    match state
        .valkey
        .check_subnet_amount_window(&subject, amount, window.max_amount, window.window_seconds)
        .await
    {
        Ok(AmountWindowDecision::Allowed {
            current,
            attempted,
            max,
            window_seconds,
        }) => {
            info!(
                subject,
                current_sent_nanograms = current,
                attempted_amount = attempted,
                max_amount = max,
                window_seconds,
                "Subnet amount sliding window checked"
            );
            Ok(Some(subject))
        }
        Ok(AmountWindowDecision::Limited {
            current,
            attempted,
            max,
            window_seconds,
            retry_after_ms,
        }) => {
            state
                .record_antifraud_trigger(AntifraudModule::SubnetAmountWindow)
                .await;
            warn!(
                subject,
                current_sent_nanograms = current,
                attempted_amount = attempted,
                max_amount = max,
                window_seconds,
                retry_after_ms,
                "Subnet amount sliding window limit reached"
            );
            Err(response_error(
                StatusCode::TOO_MANY_REQUESTS,
                "Subnet amount limit exceeded",
            ))
        }
        Err(err) => {
            error!(
                subject,
                error = %err,
                "Failed to check subnet amount window"
            );
            Err(response_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to check subnet amount limit",
            ))
        }
    }
}

// TODO: сделать по другому
async fn check_successful_claim_window(
    state: &AppState,
    subject: &str,
    max_requests: u32,
) -> ClaimLimitResult {
    let Some(window) = state.antifraud.successful_claim_window() else {
        return Ok(());
    };

    match state
        .valkey
        .check_successful_claim_window(subject, max_requests, window.window_seconds)
        .await
    {
        Ok(SuccessfulClaimWindowDecision::Allowed {
            current,
            max,
            window_seconds,
        }) => {
            info!(
                subject,
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
                subject,
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
                subject,
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

    use super::{CreateClaim, CreateClaimRequest, FaucetTier};

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

    #[test]
    fn keeps_queued_claims_from_before_github_limits_compatible() {
        let claim: CreateClaim = serde_json::from_value(json!({
            "address": "0:abc",
            "challenge": "challenge",
            "nonce": 42,
        }))
        .unwrap();

        assert_eq!(claim.github_user_id, None);
        assert_eq!(claim.tier, FaucetTier::Guest);
        assert_eq!(claim.max_requests, 0);
        assert_eq!(claim.client_window_subject, None);
        assert_eq!(claim.device_window_subject, None);
        assert_eq!(claim.subnet_amount_window_subject, None);
    }
}
