//! Loopback control API, independent of both Studio and the network containers.

mod routes;

use crate::{Error, Runtime, ServiceDescriptor, storage};
use axum::{
    Router,
    extract::{Request, State},
    http::{StatusCode, header},
    middleware::{self, Next},
    response::{IntoResponse, Response},
};
use std::{path::Path, sync::Arc};
use tokio::sync::Notify;

#[derive(Clone)]
struct ApiState {
    runtime: Runtime,
    token: String,
    shutdown: Arc<Notify>,
}

/// Builds the authenticated control API. Callers own the listening socket and
/// must call `Runtime::shutdown` before dropping the service's filesystem lock.
pub fn router(runtime: Runtime, token: String, shutdown: Arc<Notify>) -> Router {
    let state = ApiState {
        runtime,
        token,
        shutdown,
    };
    routes::router()
        .layer(middleware::from_fn_with_state(state.clone(), authorize))
        .with_state(state)
}

async fn authorize(State(state): State<ApiState>, request: Request, next: Next) -> Response {
    let authorized = request
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .is_some_and(|token| token == state.token);
    if !authorized {
        return (StatusCode::UNAUTHORIZED, axum::Json(serde_json::json!({"code":"unauthorized", "message":"A localnet service token is required"}))).into_response();
    }
    next.run(request).await
}

/// Publishes private discovery data and serves until a signal or shutdown request.
///
/// The root is one network's directory, not the project catalog. Termination stops
/// only that network while preserving its volumes. The listener must use loopback.
pub async fn serve(
    root: &Path,
    listener: tokio::net::TcpListener,
    signal: impl Future<Output = ()> + Send + 'static,
) -> Result<(), Error> {
    let address = listener
        .local_addr()
        .map_err(|e| Error::invalid(e.to_string()))?;
    if !address.ip().is_loopback() {
        return Err(Error::invalid(
            "The localnet control service requires a loopback address",
        ));
    }

    let runtime = Runtime::open(root).await?;
    // Finish the first observation before publishing discovery. A first start
    // request must not race the monitor for the network's mutation lock.
    runtime.reconcile().await;
    let token = format!(
        "{}{}",
        uuid::Uuid::new_v4().simple(),
        uuid::Uuid::new_v4().simple()
    );
    let descriptor = ServiceDescriptor {
        protocol_version: 1,
        url: format!("http://{address}"),
        token: token.clone(),
        pid: std::process::id(),
    };

    let descriptor_path = storage::service_descriptor_path(root);
    storage::write_json(&descriptor_path, &descriptor).await?;
    let shutdown = Arc::new(Notify::new());
    let app = router(runtime.clone(), token, Arc::clone(&shutdown));
    // Opening the control API observes this network without starting Docker.
    let monitor_runtime = runtime.clone();
    let monitor = tokio::spawn(async move {
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(3)).await;
            monitor_runtime.reconcile().await;
        }
    });
    let stopping_runtime = runtime.clone();
    let serve_result = axum::serve(listener, app)
        .with_graceful_shutdown(async move {
            tokio::select! { _ = signal => {}, _ = shutdown.notified() => {} }
            let _ = stopping_runtime.prepare_shutdown().await;
        })
        .await;
    monitor.abort();
    let _ = monitor.await;
    let shutdown_result = runtime.shutdown().await;
    if let Err(error) = tokio::fs::remove_file(&descriptor_path).await {
        log::warn!("operation=service_shutdown outcome=descriptor_cleanup_failed error={error}");
    }
    serve_result.map_err(|e| Error::Internal {
        code: "service_failed",
        message: e.to_string(),
    })?;
    shutdown_result
}

impl IntoResponse for Error {
    fn into_response(self) -> Response {
        let status =
            StatusCode::from_u16(self.status()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
        (
            status,
            axum::Json(serde_json::json!({"code": self.code(), "message": self.to_string()})),
        )
            .into_response()
    }
}
