use axum::{Json, extract::State, response::IntoResponse};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::{
    blockchain::{is_valid_code_hash, normalize_code_hash},
    error::ApiError,
    payment::PaymentError,
    registry::VerifiedBundleRequest,
    state::AppState,
};

#[utoipa::path(
    post,
    path = "/api/v1/take-ticket",
    operation_id = "take_ticket",
    request_body = TakeTicketRequest,
    responses(
        (status = 200, description = "Verification status or testnet payment quote", body = TakeTicketResponse),
        (status = 400, description = "Invalid code hash", body = crate::error::ErrorResponse),
        (status = 502, description = "Verification registry failure", body = crate::error::ErrorResponse),
        (status = 503, description = "Payment history recovery is in progress", body = crate::error::ErrorResponse)
    ),
    tag = "verification"
)]
pub async fn handler(
    State(state): State<AppState>,
    Json(request): Json<TakeTicketRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let code_hash = normalize_code_hash(request.code_hash.trim());
    if !is_valid_code_hash(&code_hash) {
        return Err(ApiError::bad_request(
            "code_hash must contain exactly 64 hexadecimal characters".to_owned(),
        ));
    }

    if let Some(bundle) = state
        .verification_registry()
        .verified_bundle(VerifiedBundleRequest {
            code_hash: code_hash.clone(),
        })
        .await?
        .bundle
    {
        return Ok(Json(TakeTicketResponse::AlreadyVerified {
            code_hash,
            source_bundle_hash: bundle.manifest.source_bundle_hash,
            storage_revision: bundle.storage_revision,
        }));
    }

    if !state.payment_verifier().is_ready() {
        return Err(PaymentError::RecoveryInProgress.into());
    }

    let quote = state.payment_verifier().quote(&code_hash);
    Ok(Json(TakeTicketResponse::PaymentRequired {
        code_hash,
        payment_address: quote.payment_address,
        amount_nano: quote.amount_nano,
        comment: quote.comment,
    }))
}

#[derive(Debug, Deserialize, ToSchema)]
pub(super) struct TakeTicketRequest {
    #[schema(example = "a873d8c2d163f7fa10bbe38769706f0554505e8ea2dcea3f115288db8becf2ab")]
    code_hash: String,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(tag = "status", rename_all = "snake_case")]
pub(super) enum TakeTicketResponse {
    AlreadyVerified {
        code_hash: String,
        source_bundle_hash: String,
        storage_revision: String,
    },
    PaymentRequired {
        code_hash: String,
        payment_address: String,
        amount_nano: String,
        comment: String,
    },
}
