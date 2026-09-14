use axum::{extract::State, http::StatusCode};
use tracing::error;

use crate::AppState;

pub(super) async fn health(State(state): State<AppState>) -> StatusCode {
    let (valkey_result, database_result) = tokio::join!(
        state.valkey.ping(),
        // TODO: Move the SQLite health check into a dedicated component.
        sqlx::query("SELECT 1").execute(&state.database),
    );

    if let Err(error) = &valkey_result {
        error!(%error, "Valkey health check failed");
    }
    if let Err(error) = &database_result {
        error!(%error, "SQLite health check failed");
    }

    if valkey_result.is_ok() && database_result.is_ok() {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    }
}
