use axum::{
    Json, Router,
    routing::{get, post},
};
use utoipa::OpenApi;

use crate::{error::ErrorResponse, state::AppState};

mod take_ticket;
mod verification;
mod verify;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/openapi.json", get(openapi_handler))
        .route("/last_verified", get(verification::last_verified_handler))
        .route("/statistics", get(verification::statistics_handler))
        .route(
            "/statistics/history",
            get(verification::statistics_history_handler),
        )
        .route("/abi", get(verification::abi_handler))
        .route("/take-ticket", post(take_ticket::handler))
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
        take_ticket::handler,
        verification::last_verified_handler,
        verification::statistics_handler,
        verification::statistics_history_handler,
        verification::abi_handler,
        verification::status_handler,
        verification::source_handler
    ),
    components(schemas(
        ErrorResponse,
        take_ticket::TakeTicketRequest,
        take_ticket::TakeTicketResponse,
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
        verification::VerificationStatisticsResponse,
        verification::VerificationStatisticsHistoryResponse,
        verification::VerificationStatisticsHistoryItemResponse,
        verification::LanguageStatisticsResponse,
        verification::CompilerVersionStatisticsResponse,
        verification::AbiContractsResponse,
        verification::AbiContractResponse
    )),
    tags(
        (name = "verification", description = "Source verification and lookup endpoints")
    )
)]
struct ApiDoc;
