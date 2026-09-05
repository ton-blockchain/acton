//! Inspection never launches a control service or changes Docker resources.

use super::{output, progress::Activity};
use acton_localnet::{OperationStatus, client::Client, inspection};
use reqwest::Method;
use serde_json::json;
use std::path::Path;

pub(super) async fn status(root: &Path, json: bool) -> anyhow::Result<()> {
    let mut activity = Activity::new(json);
    activity.update("Checking", "network status", None);
    let network = match Client::connect(root).await {
        Ok(client) => match client.request(Method::GET, "/v1/network", None).await {
            Ok(network) => network,
            Err(_) => inspection::status(root).await?,
        },
        Err(_) => inspection::status(root).await?,
    };
    drop(activity);
    output::network(&network, json)
}

pub(super) async fn logs(root: &Path, tail: usize, json: bool) -> anyhow::Result<()> {
    let logs = inspection::logs(root, tail).await?;
    if json {
        output::print(&json!({"logs": logs}))
    } else {
        println!("{logs}");
        Ok(())
    }
}

pub(super) async fn operation(root: &Path, id: &str, wait: bool, json: bool) -> anyhow::Result<()> {
    let mut operation = inspection::operation(root, id).await?;
    if wait && operation.status == OperationStatus::Running {
        let client = Client::connect(root).await.map_err(|_| anyhow::anyhow!(
            "Operation {id} is still recorded as running, but its service is unavailable; inspect the network logs"
        ))?;
        operation = output::wait(&client, operation, json).await?;
    }
    if wait && operation.status == OperationStatus::Failed {
        if json {
            output::print(&operation)?;
        }
        anyhow::bail!(
            "{}",
            operation
                .error
                .as_deref()
                .unwrap_or("Localnet operation failed")
        );
    }
    output::print(&operation)
}
