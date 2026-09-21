//! Selects a recent masterchain starting block through `LiteServer`.

use std::{path::Path, time::Duration};

use anyhow::{Context, Result};
use tokio::time::{Instant, timeout};
use ton_indexer_liteserver::TonutilsLiteClient;
use ton_p2p::NetworkConfig;
use tracing::info;

/// Selects a recent masterchain anchor only when there is no download checkpoint.
///
/// `LiteServer` connections are dropped before this returns; subsequent block and
/// proof downloads use P2P. The server's reported zerostate must match the config,
/// but the starting block remains a trusted anchor without consensus validation.
/// An indexer with its own checkpoint must use that ID instead of calling this.
pub async fn use_latest_block(
    config: &mut NetworkConfig,
    global_config: &Path,
    data_dir: &Path,
    request_timeout: Duration,
) -> Result<()> {
    let checkpoint = data_dir.join("checkpoint.json");
    if checkpoint
        .try_exists()
        .with_context(|| format!("cannot inspect checkpoint {}", checkpoint.display()))?
    {
        // Storage validates and locks this checkpoint when the source opens.
        // Never contact LiteServer or change the anchor of an existing session.
        info!(
            operation = "p2p_start",
            target = %checkpoint.display(),
            duration_ms = 0,
            outcome = "resuming",
            "resuming the saved P2P checkpoint",
        );
        return Ok(());
    }

    let started = Instant::now();
    info!(
        operation = "p2p_start",
        target = %global_config.display(),
        outcome = "resolving",
        "resolving a recent masterchain anchor through LiteServer",
    );

    let zero_state = config.zero_state();
    let latest = timeout(request_timeout, async {
        let mut client = TonutilsLiteClient::connect_path(global_config).await?;
        client.latest_for_network(zero_state.into()).await
    })
    .await
    .context("LiteServer bootstrap timed out")?
    .context("cannot resolve the latest masterchain block for P2P bootstrap")?;
    config.set_initial_block(latest.try_into()?)?;

    info!(
        operation = "p2p_start",
        target = %config.initial_block(),
        duration_ms = started.elapsed().as_millis(),
        outcome = "resolved",
        "selected recent starting block; block downloads will use P2P",
    );
    Ok(())
}
