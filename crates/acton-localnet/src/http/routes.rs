//! Thin HTTP adapters. All mutation ordering belongs to the runtime.

use super::ApiState;
use crate::{Error, Operation, runtime::Action};
use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::StatusCode,
    routing::{delete, get, post},
};
use serde::Deserialize;
use serde_json::{Value, json};

pub(super) fn router() -> Router<ApiState> {
    Router::new()
        .route(
            "/v1/health",
            get(|| async { Json(json!({"protocolVersion": 1, "service": "acton-localnet"})) }),
        )
        .route("/v1/shutdown", post(shutdown))
        .route("/v1/network", get(network).delete(remove))
        .route("/v1/network/health", get(network_health))
        .route(
            "/v1/network/admin",
            get(admin_operation)
                .post(start_admin)
                .layer(axum::extract::DefaultBodyLimit::max(16 * 1024 * 1024)),
        )
        .route("/v1/network/start", post(start))
        .route("/v1/network/stop", post(stop))
        .route("/v1/network/logs", get(logs))
        .route("/v1/network/nodes", post(add_node))
        .route("/v1/network/nodes/{node}", delete(remove_node))
        .route(
            "/v1/network/nodes/{node}/enter-validation",
            post(enter_validation),
        )
        .route(
            "/v1/network/nodes/{node}/leave-validation",
            post(leave_validation),
        )
        .route(
            "/v1/network/snapshots",
            get(snapshots).post(create_snapshot),
        )
        .route("/v1/network/snapshots/{snapshot}", delete(delete_snapshot))
        .route(
            "/v1/network/snapshots/{snapshot}/restore",
            post(restore_snapshot),
        )
        .route("/v1/operations/{id}", get(operation))
}

async fn network(State(state): State<ApiState>) -> Json<crate::Network> {
    Json(state.runtime.get().await)
}

async fn network_health(
    State(state): State<ApiState>,
) -> Result<Json<crate::NetworkHealth>, Error> {
    state.runtime.health().await.map(Json)
}

async fn operation(
    State(state): State<ApiState>,
    Path(id): Path<String>,
) -> Result<Json<Operation>, Error> {
    state.runtime.operation(&id).await.map(Json)
}

async fn accepted(state: ApiState, action: Action) -> Result<(StatusCode, Json<Operation>), Error> {
    // A request acknowledges ownership of the operation, not its completion.
    // Clients poll the durable record after disconnecting or changing pages.
    state
        .runtime
        .submit(action)
        .await
        .map(|op| (StatusCode::ACCEPTED, Json(op)))
}

async fn start(State(state): State<ApiState>) -> Result<(StatusCode, Json<Operation>), Error> {
    accepted(state, Action::Start).await
}

async fn stop(State(state): State<ApiState>) -> Result<(StatusCode, Json<Operation>), Error> {
    accepted(state, Action::Stop).await
}

async fn remove(State(state): State<ApiState>) -> Result<(StatusCode, Json<Operation>), Error> {
    accepted(state, Action::Delete).await
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AddNode {
    name: String,
    #[serde(default)]
    validator: bool,
}

async fn add_node(
    State(state): State<ApiState>,
    Json(request): Json<AddNode>,
) -> Result<(StatusCode, Json<Operation>), Error> {
    accepted(
        state,
        Action::AddNode {
            name: request.name,
            validator: request.validator,
        },
    )
    .await
}

#[derive(Deserialize)]
struct RemoveNode {
    #[serde(default)]
    force: bool,
}

async fn remove_node(
    State(state): State<ApiState>,
    Path(node): Path<String>,
    Query(request): Query<RemoveNode>,
) -> Result<(StatusCode, Json<Operation>), Error> {
    accepted(
        state,
        Action::RemoveNode {
            id: node,
            force: request.force,
        },
    )
    .await
}

async fn enter_validation(
    State(state): State<ApiState>,
    Path(node): Path<String>,
) -> Result<(StatusCode, Json<Operation>), Error> {
    accepted(
        state,
        Action::Validation {
            id: node,
            enabled: true,
        },
    )
    .await
}

async fn leave_validation(
    State(state): State<ApiState>,
    Path(node): Path<String>,
) -> Result<(StatusCode, Json<Operation>), Error> {
    accepted(
        state,
        Action::Validation {
            id: node,
            enabled: false,
        },
    )
    .await
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SnapshotName {
    name: Option<String>,
}

async fn create_snapshot(
    State(state): State<ApiState>,
    Json(request): Json<SnapshotName>,
) -> Result<(StatusCode, Json<Operation>), Error> {
    accepted(state, Action::CreateSnapshot { name: request.name }).await
}

async fn delete_snapshot(
    State(state): State<ApiState>,
    Path(snapshot): Path<String>,
) -> Result<(StatusCode, Json<Operation>), Error> {
    accepted(state, Action::DeleteSnapshot { id: snapshot }).await
}

async fn restore_snapshot(
    State(state): State<ApiState>,
    Path(snapshot): Path<String>,
) -> Result<(StatusCode, Json<Operation>), Error> {
    accepted(state, Action::RestoreSnapshot { id: snapshot }).await
}

async fn snapshots(State(state): State<ApiState>) -> Result<Json<Vec<crate::Snapshot>>, Error> {
    state.runtime.snapshots().await.map(Json)
}

#[derive(Deserialize)]
struct LogQuery {
    tail: Option<usize>,
}

async fn logs(
    State(state): State<ApiState>,
    Query(query): Query<LogQuery>,
) -> Result<Json<Value>, Error> {
    state
        .runtime
        .logs(query.tail.unwrap_or(100).min(2000))
        .await
        .map(|logs| Json(json!({"logs":logs})))
}

async fn shutdown(State(state): State<ApiState>) -> Result<StatusCode, Error> {
    state.runtime.prepare_shutdown().await?;
    state.shutdown.notify_one();
    Ok(StatusCode::ACCEPTED)
}

async fn admin_operation(
    State(state): State<ApiState>,
) -> Result<Json<Option<crate::AdminOperation>>, Error> {
    state.runtime.admin_operation().await.map(Json)
}

async fn start_admin(
    State(state): State<ApiState>,
    Json(request): Json<crate::AdminRequest>,
) -> Result<(StatusCode, Json<crate::AdminOperation>), Error> {
    state
        .runtime
        .start_admin(request)
        .await
        .map(|op| (StatusCode::ACCEPTED, Json(op)))
}
