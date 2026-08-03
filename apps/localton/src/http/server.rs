//! Runs one Axum router as a supervised HTTP task.
//!
//! [`start`] binds the requested socket, spawns `axum::serve`, and returns its
//! task handle together with the endpoint published in runtime state. The task
//! stops after the shared watch channel changes or closes and logs server errors
//! with the supplied service name.

use std::net::SocketAddr;

use anyhow::{Context, Result};
use axum::Router;
use tokio::{net::TcpListener, sync::watch};
use tracing::warn;

use super::RunningService;

pub(super) async fn start(
    name: &'static str,
    address: SocketAddr,
    app: Router,
    mut shutdown: watch::Receiver<bool>,
    endpoint: String,
) -> Result<RunningService> {
    let listener = TcpListener::bind(address)
        .await
        .with_context(|| format!("failed to bind {name} to {address}"))?;
    let task = tokio::spawn(async move {
        let result = axum::serve(listener, app)
            .with_graceful_shutdown(async move {
                while !*shutdown.borrow() {
                    if shutdown.changed().await.is_err() {
                        break;
                    }
                }
            })
            .await;
        if let Err(error) = result {
            warn!(service = name, %error, "HTTP service stopped with an error");
        }
    });
    Ok(RunningService { task, endpoint })
}
