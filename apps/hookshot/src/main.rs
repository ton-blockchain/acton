mod config;

use std::time::Duration;

use anyhow::Context;
use axum::{Router, routing::get};
use config::Config;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let config = Config::load()?;

    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_new(&config.logging.level).context("invalid logging.level")?,
        )
        .init();

    let http_client = reqwest::Client::builder()
        .user_agent(concat!(
            env!("CARGO_PKG_NAME"),
            "/",
            env!("CARGO_PKG_VERSION")
        ))
        .timeout(Duration::from_secs(config.http.timeout_seconds))
        .build()
        .context("failed to create HTTP client")?;

    let app = Router::new()
        .route("/healthz", get(|| async { "ok" }))
        .with_state(http_client);

    let listener = tokio::net::TcpListener::bind(config.server.bind_addr)
        .await
        .with_context(|| format!("failed to bind HTTP server to {}", config.server.bind_addr))?;
    tracing::info!(address = %listener.local_addr()?, "starting Hookshot");

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .context("HTTP server failed")
}

async fn shutdown_signal() {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{SignalKind, signal};

        match signal(SignalKind::terminate()) {
            Ok(mut terminate) => {
                tokio::select! {
                    result = tokio::signal::ctrl_c() => {
                        if let Err(error) = result {
                            tracing::error!(%error, "failed to listen for Ctrl-C");
                        }
                    }
                    _ = terminate.recv() => {}
                }
                tracing::info!("shutdown requested; waiting for active requests");
                return;
            }
            Err(error) => tracing::error!(%error, "failed to install SIGTERM handler"),
        }
    }

    if let Err(error) = tokio::signal::ctrl_c().await {
        tracing::error!(%error, "failed to listen for Ctrl-C");
    }
    tracing::info!("shutdown requested; waiting for active requests");
}
