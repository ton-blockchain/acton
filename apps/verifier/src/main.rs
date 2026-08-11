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
        network = %config.network(),
        toncenter_base_url = %config.toncenter_base_url(),
        "starting verifier backend"
    );

    let state = AppState::from_config(&config)?;
    if config.source_repository_path().is_some() {
        state.ensure_registry_current().await?;
    }

    let payment_state = state.clone();
    tokio::spawn(async move {
        let mut retry_delay = std::time::Duration::from_secs(1);
        loop {
            match payment_state.recover_payment_history().await {
                Ok(()) => break,
                Err(error) => {
                    tracing::error!(%error, "failed to recover payment history; retrying");
                    tokio::time::sleep(retry_delay).await;
                    retry_delay = (retry_delay * 2).min(std::time::Duration::from_secs(30));
                }
            }
        }
    });

    axum::serve(listener, app::router_with_state(state)).await?;

    Ok(())
}
