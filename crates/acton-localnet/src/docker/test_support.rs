//! Real Docker probes shared by node and snapshot regression scenarios.

use super::DockerNetwork;
use crate::{Operation, OperationStatus, Runtime, runtime::Action};
use anyhow::{Context as _, Result, ensure};
use serde_json::{Value, json};
use std::time::Duration;

pub(super) async fn operation(runtime: &Runtime, action: Action) -> Result<Operation> {
    let accepted = runtime.submit(action).await?;
    eprintln!("Running {}", accepted.kind);
    tokio::time::timeout(Duration::from_secs(720), async {
        loop {
            // Progress and inventory must remain readable while the mutation is held.
            runtime.snapshots().await?;
            let operation = runtime.operation(&accepted.id).await?;
            if operation.status != OperationStatus::Running {
                eprintln!("{}: {:?}", operation.kind, operation.status);
                return Ok(operation);
            }
            tokio::time::sleep(Duration::from_millis(200)).await;
        }
    })
    .await?
}

pub(super) async fn completed(runtime: &Runtime, action: Action) -> Result<Value> {
    let operation = operation(runtime, action).await?;
    ensure!(
        operation.status == OperationStatus::Completed,
        "{:?}",
        operation.error
    );
    Ok(operation.result.unwrap_or(Value::Null))
}

async fn lite(driver: &DockerNetwork, service: &str, query: &str) -> Result<String> {
    // Address each node's own liteserver. The pinned image's generic CLI can
    // select the downloaded bootstrap config instead of the joined node config.
    let mut command = driver.compose_command();
    command.args([
        "exec",
        "-T",
        service,
        "/opt/ton/lite-client",
        "-v",
        "0",
        "-t",
        "10",
        "-C",
        "/var/lib/localton/node/global.config.json",
        "-c",
        query,
    ]);
    command.kill_on_drop(true);
    let output = tokio::time::timeout(Duration::from_secs(15), command.output())
        .await
        .with_context(|| format!("Liteserver query timed out on {service}"))??;
    let text = format!(
        "{}\n{}",
        String::from_utf8(output.stdout)?,
        String::from_utf8(output.stderr)?
    );
    ensure!(
        output.status.success(),
        "Liteserver query {query} failed on {service} ({}): {text}",
        output.status
    );
    Ok(text)
}

pub(super) async fn head(driver: &DockerNetwork, service: &str) -> Result<u32> {
    let output = lite(driver, service, "last").await?;
    output
        .split("(-1,8000000000000000,")
        .skip(1)
        .filter_map(|block| block.split(')').next()?.parse().ok())
        .max()
        .with_context(|| format!("Masterchain head missing on {service}: {output}"))
}

pub(super) async fn block(driver: &DockerNetwork, service: &str, seqno: u32) -> Result<Value> {
    let output = lite(
        driver,
        service,
        &format!("byseqno -1:8000000000000000 {seqno}"),
    )
    .await?;
    let prefix = format!("(-1,8000000000000000,{seqno}):");
    let hashes = output
        .split(&prefix)
        .nth(1)
        .and_then(|value| value.split_whitespace().next())
        .with_context(|| format!("Block {seqno} identity missing on {service}: {output}"))?;
    ensure!(hashes.len() == 129, "Invalid block hashes from {service}");
    Ok(json!({"seqno": seqno, "hashes": hashes}))
}
