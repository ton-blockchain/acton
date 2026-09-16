use tracing_subscriber::EnvFilter;
use verifier::{app, config::Config, state::AppState};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = Config::load()?;

    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_new(config.logging_level())?)
        .init();

    let addr = config.bind_addr();

    let listener = tokio::net::TcpListener::bind(addr).await?;
    tracing::info!(
        %addr,
        payment_primary_network = %config.payment_primary_network(),
        read_only = config.read_only(),
        toncenter_mainnet_base_url = %config.toncenter_mainnet_base_url(),
        toncenter_testnet_base_url = %config.toncenter_testnet_base_url(),
        "starting verifier backend"
    );

    if config.read_only() {
        tracing::warn!(
            "verifier read-only mode is enabled; new contract verifications will be rejected"
        );
    }

    let state = AppState::from_config(&config)?;
    let published_payment_transaction_hashes = if config.source_repository_path().is_some() {
        state.ensure_registry_current().await?;
        state.published_payment_transaction_hashes().await?
    } else {
        Vec::new()
    };

    let payment_state = state.clone();
    tokio::spawn(async move {
        let mut retry_delay = std::time::Duration::from_secs(1);
        loop {
            match payment_state
                .recover_payment_history(&published_payment_transaction_hashes)
                .await
            {
                Ok(()) => break,
                Err(error) => {
                    tracing::error!(%error, "failed to recover payment history; retrying");
                    tokio::time::sleep(retry_delay).await;
                    retry_delay = (retry_delay * 2).min(std::time::Duration::from_secs(30));
                }
            }
        }
    });

    axum::serve(listener, app::router_with_state(state.clone()))
        .with_graceful_shutdown(shutdown_signal())
        .await?;
    state.wait_for_background_tasks().await;

    Ok(())
}

#[cfg(unix)]
async fn shutdown_signal() {
    use tokio::signal::unix::{SignalKind, signal};

    let mut terminate = match signal(SignalKind::terminate()) {
        Ok(signal) => signal,
        Err(error) => {
            tracing::error!(%error, "failed to install SIGTERM handler");
            let _ = tokio::signal::ctrl_c().await;
            return;
        }
    };
    tokio::select! {
        result = tokio::signal::ctrl_c() => {
            if let Err(error) = result {
                tracing::error!(%error, "failed to listen for Ctrl-C");
            }
        }
        _ = terminate.recv() => {}
    }
    tracing::info!("shutdown requested; waiting for active requests");
}

#[cfg(not(unix))]
async fn shutdown_signal() {
    if let Err(error) = tokio::signal::ctrl_c().await {
        tracing::error!(%error, "failed to listen for Ctrl-C");
    }
    tracing::info!("shutdown requested; waiting for active requests");
}
