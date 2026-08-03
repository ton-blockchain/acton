use actonscan_backend::{Config, SqliteStorage, app, spawn_indexer};
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let config = Config::load()?;
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::new(config.logging_level()))
        .init();

    let storage = SqliteStorage::open(config.database_path())?;
    let tps_stats = storage.load_tps_stats()?;
    let opcode_stats = storage.load_opcode_stats()?;
    tracing::info!(path = %config.database_path().display(), "opened Actonscan database");

    let _indexer = spawn_indexer(
        config.indexer().clone(),
        tps_stats.clone(),
        opcode_stats.clone(),
        storage,
    );
    let listener = tokio::net::TcpListener::bind(config.bind_addr()).await?;
    tracing::info!(address = %config.bind_addr(), "starting Actonscan backend");

    axum::serve(listener, app(tps_stats, opcode_stats)).await?;
    Ok(())
}
