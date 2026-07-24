use axum::{
    Json, Router,
    routing::{get, post},
};
use utoipa::OpenApi;

use crate::{error::ErrorResponse, state::AppState};

mod verification;
mod verify;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/openapi.json", get(openapi_handler))
        .route("/last_verified", get(verification::last_verified_handler))
        .route("/abi", get(verification::abi_handler))
        .route("/verify", post(verify::handler))
        .route("/verification/status", get(verification::status_handler))
        .route("/verification/source", get(verification::source_handler))
}

async fn openapi_handler() -> Json<utoipa::openapi::OpenApi> {
    Json(openapi())
}

fn openapi() -> utoipa::openapi::OpenApi {
    ApiDoc::openapi()
}

#[derive(OpenApi)]
#[openapi(
    info(
        title = "TON Source Verifier API",
        version = "0.1.0",
        description = "API for verifying TON smart contract source bundles by code hash."
    ),
    paths(
        verify::handler,
        verification::last_verified_handler,
        verification::abi_handler,
        verification::status_handler,
        verification::source_handler
    ),
    components(schemas(
        ErrorResponse,
        verify::SourceMetadata,
        verify::VerifyMultipartRequest,
        verify::VerifyResponse,
        verify::VerificationResult,
        verification::VerificationStatusResponse,
        verification::VerificationSourceResponse,
        verification::SourceBundleResponse,
        crate::source_storage::SourceMapData,
        verification::CompilerResponse,
        verification::SourceFileResponse,
        verification::LastVerifiedResponse,
        verification::LastVerifiedItemResponse,
        verification::AbiContractsResponse,
        verification::AbiContractResponse
    )),
    tags(
        (name = "verification", description = "Source verification and lookup endpoints")
    )
)]
struct ApiDoc;
