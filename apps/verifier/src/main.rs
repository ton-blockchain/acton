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

    axum::serve(listener, app::router_with_state(state)).await?;

    Ok(())
}
