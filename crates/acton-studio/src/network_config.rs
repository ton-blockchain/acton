//! HTTP projection of the localnet owner's on-chain config operations.

use crate::{StudioApiError, StudioState};
use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};

/// Submit one parameter change to the environment's full localnet process
#[utoipa::path(post, path = "/api/v1/environments/{environment_id}/network/config",
    params(("environment_id" = String, Path)), request_body = acton_localnet::UpdateNetworkConfig,
    responses((status = 202, description = "Accepted config operation", body = acton_localnet::Operation)), tag = "environments")]
pub(crate) async fn update(
    State(state): State<StudioState>,
    Path(environment_id): Path<String>,
    Json(request): Json<acton_localnet::UpdateNetworkConfig>,
) -> Result<(StatusCode, Json<acton_localnet::Operation>), StudioApiError> {
    let operation = state
        .environment_runtime
        .update_network_config(&environment_id, request)
        .await
        .map_err(StudioApiError)?;
    Ok((StatusCode::ACCEPTED, Json(operation)))
}

/// Inspect accepted work after navigation or reconnection, without repeating the change
#[utoipa::path(get, path = "/api/v1/environments/{environment_id}/localnet-operations/{operation_id}",
    params(("environment_id" = String, Path), ("operation_id" = String, Path)),
    responses((status = 200, description = "Durable localnet operation", body = acton_localnet::Operation)), tag = "environments")]
pub(crate) async fn operation(
    State(state): State<StudioState>,
    Path((environment_id, operation_id)): Path<(String, String)>,
) -> Result<Json<acton_localnet::Operation>, StudioApiError> {
    state
        .environment_runtime
        .localnet_operation(&environment_id, &operation_id)
        .await
        .map(Json)
        .map_err(StudioApiError)
}
