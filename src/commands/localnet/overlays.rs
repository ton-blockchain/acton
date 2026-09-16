//! File-based private overlay configuration uses the same control API as other clients.

use std::path::PathBuf;

use acton_localnet::{OverlayConfig, client::Client};
use anyhow::Context;
use reqwest::Method;

use super::output;

pub(super) async fn run(
    client: &Client,
    config: Option<PathBuf>,
    json: bool,
) -> anyhow::Result<()> {
    let Some(path) = config else {
        return output::print(&client.overlays().await?);
    };
    let contents = tokio::fs::read(&path)
        .await
        .with_context(|| format!("Failed to read overlay configuration {}", path.display()))?;
    let config: OverlayConfig = serde_json::from_slice(&contents)
        .with_context(|| format!("Invalid overlay configuration in {}", path.display()))?;
    output::mutate(
        client,
        Method::PUT,
        "/v1/network/overlays",
        Some(serde_json::to_value(config)?),
        json,
    )
    .await
}
