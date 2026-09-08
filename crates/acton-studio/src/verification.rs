//! Source verification reviews local bytes, prepares Testnet payment, then publishes after finality.
//! The runtime never signs a wallet transaction; Studio asks the selected wallet separately.

use std::future::Future;
use std::pin::Pin;

use axum::Router;
use axum::extract::{Path, Query, State};
use axum::response::Json;
use axum::routing::{get, post};
use serde::{Deserialize, Serialize};
use utoipa::{OpenApi, ToSchema};

use crate::{EnvironmentRuntimeError, PublicTonNetwork, StudioApiError, StudioState};

/// Public registry observation, separate from Studio's local source artifacts.
#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct VerificationStatus {
    pub code_hash: String,
    pub verified: bool,
    pub verifier_url: String,
}

/// A project entrypoint and its current compilation result, including recoverable build failures.
#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct VerificationCandidate {
    pub contract_id: String,
    pub source_path: String,
    pub code_hash: Option<String>,
    pub matches: bool,
    pub error: Option<String>,
    pub files: Vec<VerificationFile>,
}

/// File metadata shown before publication; source contents stay in the runtime's preview.
#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct VerificationFile {
    pub path: String,
    pub size_bytes: usize,
}

/// An expiring snapshot of project sources. Its ID binds a later upload to reviewed bytes.
#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct VerificationPreview {
    pub id: String,
    pub status: VerificationStatus,
    pub compiler_version: String,
    pub candidates: Vec<VerificationCandidate>,
    pub payment: Option<VerificationPayment>,
}

/// The new verifier charges in Testnet even when the target contract is on Mainnet.
/// Approval must display this network separately from the contract's network.
#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
pub struct VerificationPayment {
    pub address: String,
    pub amount: String,
    pub comment: String,
    pub network: PublicTonNetwork,
}

/// The wallet's normalized external message hash lets the server find the finalized
/// recipient transaction and retry publication without asking for another payment.
#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct VerificationPaymentRequest {
    pub message_hash: String,
}

/// Identifies compiled code independently of the accounts that deploy it.
#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct VerificationTarget {
    pub code_hash: String,
}

/// Explicit publication approval for a previously reviewed candidate and payer.
#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct StartVerificationRequest {
    pub preview_id: String,
    pub contract_id: String,
    pub sender_address: String,
}

/// Preparation progress. Ready means the wallet still needs to send the transaction.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub enum VerificationPhase {
    UploadingSources,
    ConfirmingPayment,
    Ready,
    Verified,
    Failed,
}

/// An unsigned internal message for the existing Studio wallet integration.
#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
pub struct VerificationMessage {
    pub address: String,
    pub amount: String,
    pub payload: String,
}

/// Pollable operation state survives dialog dismissal; an upload ID is never started twice.
#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct VerificationOperation {
    pub id: String,
    pub phase: VerificationPhase,
    pub message: Option<VerificationMessage>,
    pub error: Option<String>,
}

/// Async boundary between the HTTP server and the CLI compiler/verifier implementation.
pub type VerificationFuture<'a, T> =
    Pin<Box<dyn Future<Output = Result<T, EnvironmentRuntimeError>> + Send + 'a>>;

/// Owns bounded, immutable source previews and idempotent publication operations.
/// Implementations must reject cross-network preview reuse and never spend from a wallet.
pub trait VerificationRuntime: Send + Sync {
    fn status(
        &self,
        network: PublicTonNetwork,
        code_hash: String,
    ) -> VerificationFuture<'_, VerificationStatus>;

    fn preview(
        &self,
        network: PublicTonNetwork,
        code_hash: String,
    ) -> VerificationFuture<'_, VerificationPreview>;

    fn start(
        &self,
        network: PublicTonNetwork,
        request: StartVerificationRequest,
    ) -> VerificationFuture<'_, VerificationOperation>;

    fn operation(
        &self,
        network: PublicTonNetwork,
        id: String,
    ) -> VerificationFuture<'_, VerificationOperation>;

    fn complete_payment(
        &self,
        network: PublicTonNetwork,
        id: String,
        request: VerificationPaymentRequest,
    ) -> VerificationFuture<'_, VerificationOperation>;
}

pub(super) fn router() -> Router<StudioState> {
    Router::new()
        .route(
            "/environments/{environment_id}/verification/status",
            get(status),
        )
        .route(
            "/environments/{environment_id}/verification/preview",
            post(preview),
        )
        .route(
            "/environments/{environment_id}/verification/operations",
            post(start),
        )
        .route(
            "/environments/{environment_id}/verification/operations/{id}",
            get(operation),
        )
        .route(
            "/environments/{environment_id}/verification/operations/{id}/payment",
            post(complete_payment),
        )
}

async fn network(state: &StudioState, id: &str) -> Result<PublicTonNetwork, StudioApiError> {
    let environment = state
        .environment_runtime
        .get(id)
        .await
        .map_err(StudioApiError)?;
    crate::public_ton_network(&environment).ok_or_else(|| {
        StudioApiError(EnvironmentRuntimeError::InvalidRequest {
            code: "verification_network_unsupported",
            message: "Source verification is available on Mainnet and Testnet".to_owned(),
        })
    })
}

fn runtime(state: &StudioState) -> Result<&dyn VerificationRuntime, StudioApiError> {
    state.verification_runtime.as_deref().ok_or_else(|| {
        StudioApiError(EnvironmentRuntimeError::Conflict {
            code: "verification_project_required",
            message: "Open Studio from an Acton project to verify its Tolk contracts".to_owned(),
        })
    })
}

#[utoipa::path(
    get,
    path = "/api/v1/environments/{environment_id}/verification/status",
    params(
        ("environment_id" = String, Path),
        ("codeHash" = String, Query)
    ),
    responses(
        (status = 200, body = VerificationStatus),
        (status = 400, description = "Unsupported network or invalid request", body = crate::StudioApiErrorBody),
        (status = 409, description = "Project or preview is unavailable", body = crate::StudioApiErrorBody),
        (status = 500, description = "Verification failed", body = crate::StudioApiErrorBody)
    ),
    tag = "verification"
)]
async fn status(
    State(state): State<StudioState>,
    Path(id): Path<String>,
    Query(target): Query<VerificationTarget>,
) -> Result<Json<VerificationStatus>, StudioApiError> {
    let network = network(&state, &id).await?;
    runtime(&state)?
        .status(network, target.code_hash)
        .await
        .map(Json)
        .map_err(StudioApiError)
}

#[utoipa::path(
    post,
    path = "/api/v1/environments/{environment_id}/verification/preview",
    params(
        ("environment_id" = String, Path)
    ),
    request_body = VerificationTarget,
    responses(
        (status = 200, body = VerificationPreview),
        (status = 400, description = "Unsupported network or invalid request", body = crate::StudioApiErrorBody),
        (status = 409, description = "Project or preview is unavailable", body = crate::StudioApiErrorBody),
        (status = 500, description = "Verification failed", body = crate::StudioApiErrorBody)
    ),
    tag = "verification"
)]
async fn preview(
    State(state): State<StudioState>,
    Path(id): Path<String>,
    Json(target): Json<VerificationTarget>,
) -> Result<Json<VerificationPreview>, StudioApiError> {
    let network = network(&state, &id).await?;
    runtime(&state)?
        .preview(network, target.code_hash)
        .await
        .map(Json)
        .map_err(StudioApiError)
}

#[utoipa::path(
    post,
    path = "/api/v1/environments/{environment_id}/verification/operations",
    params(
        ("environment_id" = String, Path)
    ),
    request_body = StartVerificationRequest,
    responses(
        (status = 200, body = VerificationOperation),
        (status = 400, description = "Unsupported network or invalid request", body = crate::StudioApiErrorBody),
        (status = 409, description = "Project or preview is unavailable", body = crate::StudioApiErrorBody),
        (status = 500, description = "Verification failed", body = crate::StudioApiErrorBody)
    ),
    tag = "verification"
)]
async fn start(
    State(state): State<StudioState>,
    Path(id): Path<String>,
    Json(request): Json<StartVerificationRequest>,
) -> Result<Json<VerificationOperation>, StudioApiError> {
    let network = network(&state, &id).await?;
    runtime(&state)?
        .start(network, request)
        .await
        .map(Json)
        .map_err(StudioApiError)
}

#[utoipa::path(
    get,
    path = "/api/v1/environments/{environment_id}/verification/operations/{id}",
    params(
        ("environment_id" = String, Path),
        ("id" = String, Path)
    ),
    responses(
        (status = 200, body = VerificationOperation),
        (status = 400, description = "Unsupported network or invalid request", body = crate::StudioApiErrorBody),
        (status = 409, description = "Project or preview is unavailable", body = crate::StudioApiErrorBody),
        (status = 500, description = "Verification failed", body = crate::StudioApiErrorBody)
    ),
    tag = "verification"
)]
async fn operation(
    State(state): State<StudioState>,
    Path((environment_id, id)): Path<(String, String)>,
) -> Result<Json<VerificationOperation>, StudioApiError> {
    let network = network(&state, &environment_id).await?;
    runtime(&state)?
        .operation(network, id)
        .await
        .map(Json)
        .map_err(StudioApiError)
}

#[utoipa::path(
    post,
    path = "/api/v1/environments/{environment_id}/verification/operations/{id}/payment",
    params(
        ("environment_id" = String, Path),
        ("id" = String, Path)
    ),
    request_body = VerificationPaymentRequest,
    responses(
        (status = 200, body = VerificationOperation),
        (status = 400, description = "Invalid payment reference", body = crate::StudioApiErrorBody),
        (status = 409, description = "Operation or payment is unavailable", body = crate::StudioApiErrorBody),
        (status = 500, description = "Verification failed", body = crate::StudioApiErrorBody)
    ),
    tag = "verification"
)]
async fn complete_payment(
    State(state): State<StudioState>,
    Path((environment_id, id)): Path<(String, String)>,
    Json(request): Json<VerificationPaymentRequest>,
) -> Result<Json<VerificationOperation>, StudioApiError> {
    let network = network(&state, &environment_id).await?;
    runtime(&state)?
        .complete_payment(network, id, request)
        .await
        .map(Json)
        .map_err(StudioApiError)
}

#[derive(OpenApi)]
#[openapi(
    paths(status, preview, start, operation, complete_payment),
    components(schemas(
        VerificationStatus,
        VerificationCandidate,
        VerificationFile,
        VerificationPreview,
        VerificationTarget,
        StartVerificationRequest,
        VerificationPhase,
        VerificationMessage,
        VerificationOperation,
        VerificationPayment,
        VerificationPaymentRequest
    ))
)]
struct VerificationApiDoc;

pub(super) fn openapi() -> utoipa::openapi::OpenApi {
    VerificationApiDoc::openapi()
}
