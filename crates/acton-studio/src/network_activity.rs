//! Studio projects activity controls; the localnet process owns all execution.

use crate::{StudioApiError, StudioState};
use acton_localnet::activity::{ActivityCommand, ActivityConfig, ActivityState};
use axum::{
    Json,
    extract::{Path, State},
};

#[utoipa::path(
    get,
    path = "/api/v1/environments/{environment_id}/network/activity",
    params(("environment_id" = String, Path)),
    responses((status = 200, description = "Saved settings and current activity", body = ActivityState)),
    tag = "environments"
)]
pub(crate) async fn get(
    State(state): State<StudioState>,
    Path(environment_id): Path<String>,
) -> Result<Json<ActivityState>, StudioApiError> {
    control(state, environment_id, None).await
}

#[utoipa::path(
    put,
    path = "/api/v1/environments/{environment_id}/network/activity",
    params(("environment_id" = String, Path)),
    request_body = ActivityConfig,
    responses((status = 200, description = "Saved activity settings", body = ActivityState)),
    tag = "environments"
)]
pub(crate) async fn save(
    State(state): State<StudioState>,
    Path(environment_id): Path<String>,
    Json(config): Json<ActivityConfig>,
) -> Result<Json<ActivityState>, StudioApiError> {
    control(state, environment_id, Some(ActivityCommand::Save(config))).await
}

#[utoipa::path(
    post,
    path = "/api/v1/environments/{environment_id}/network/activity/start",
    params(("environment_id" = String, Path)),
    request_body = ActivityConfig,
    responses((status = 200, description = "Activity started", body = ActivityState)),
    tag = "environments"
)]
pub(crate) async fn start(
    State(state): State<StudioState>,
    Path(environment_id): Path<String>,
    Json(config): Json<ActivityConfig>,
) -> Result<Json<ActivityState>, StudioApiError> {
    control(state, environment_id, Some(ActivityCommand::Start(config))).await
}

#[utoipa::path(
    post,
    path = "/api/v1/environments/{environment_id}/network/activity/stop",
    params(("environment_id" = String, Path)),
    responses((status = 200, description = "Activity stopped", body = ActivityState)),
    tag = "environments"
)]
pub(crate) async fn stop(
    State(state): State<StudioState>,
    Path(environment_id): Path<String>,
) -> Result<Json<ActivityState>, StudioApiError> {
    control(state, environment_id, Some(ActivityCommand::Stop)).await
}

async fn control(
    state: StudioState,
    environment_id: String,
    command: Option<ActivityCommand>,
) -> Result<Json<ActivityState>, StudioApiError> {
    state
        .environment_runtime
        .network_activity(&environment_id, command)
        .await
        .map(Json)
        .map_err(StudioApiError)
}
