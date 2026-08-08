//! Readiness checks and termination signals for a running local network.
//!
//! Process creation alone does not mean that TON is usable. The launcher polls
//! the liteserver until two distinct masterchain seqnos are observed while also
//! checking that every managed process remains alive. After startup, the same
//! registry is supervised until Ctrl-C, SIGTERM, or a child failure occurs.

use std::time::Duration;

use anyhow::{Context, Result, bail};
use regex::Regex;
use tokio::{process::Command, signal, time::sleep};
use tracing::info;

use crate::{
    binaries::TonBinaries,
    runtime::{ProcessRegistry, run_checked},
    storage::{Layout, Manifest},
};

/// Proves that the liteserver is reachable and the masterchain is advancing.
///
/// A single positive seqno could be stale state from an earlier run. The launcher
/// therefore waits for a later seqno greater than the first observation while
/// checking required child processes on every iteration. Success means both the
/// query path and ongoing block production work.
pub(super) async fn wait_for_blocks(
    layout: &Layout,
    binaries: &TonBinaries,
    manifest: &Manifest,
    processes: &ProcessRegistry,
    timeout: Duration,
) -> Result<()> {
    info!("waiting for liteserver and masterchain block production");
    let deadline = tokio::time::Instant::now() + timeout;
    let mut first_seqno = None;
    loop {
        processes.ensure_alive().await?;
        if let Ok(seqno) = lite_client_seqno(binaries, manifest).await {
            match first_seqno {
                None if seqno > 0 => first_seqno = Some(seqno),
                Some(first) if seqno > first => {
                    info!(
                        first_seqno = first,
                        current_seqno = seqno,
                        "masterchain advanced"
                    );
                    return Ok(());
                }
                _ => {}
            }
        }
        if tokio::time::Instant::now() >= deadline {
            bail!(
                "masterchain did not advance within {}s; inspect {}",
                timeout.as_secs(),
                layout.logs.display()
            );
        }
        sleep(Duration::from_secs(1)).await;
    }
}

/// Queries the configured liteserver for its latest masterchain block number.
///
/// Using the persisted global config also verifies the liteserver public key and
/// zerostate identity that external clients will use after startup.
pub(super) async fn lite_client_seqno(binaries: &TonBinaries, manifest: &Manifest) -> Result<u32> {
    let mut command = Command::new(binaries.command("lite-client"));
    command
        .args(["-t", "10", "-C"])
        .arg(&manifest.global_config)
        .args(["-c", "last"]);
    let output = run_checked("lite-client last", command, Duration::from_secs(10)).await?;
    parse_masterchain_seqno(&format!("{}\n{}", output.stdout, output.stderr))
}

/// Keeps the launcher alive until one required child process exits.
///
/// The registry reports the process name and exit status as an error; the outer
/// pipeline then performs coordinated shutdown instead of leaving a partially
/// functioning network running.
pub(super) async fn supervise(processes: &ProcessRegistry) -> Result<()> {
    loop {
        sleep(Duration::from_millis(250)).await;
        processes.ensure_alive().await?;
    }
}

/// Waits for the platform's normal interactive or service-manager stop signal.
///
/// Unix handles both Ctrl-C (`SIGINT`) and `SIGTERM`, which is used by Docker and
/// process supervisors. Returning normally sends execution through the same
/// cleanup path as a child-process failure.
pub(super) async fn shutdown_signal() -> Result<()> {
    #[cfg(unix)]
    {
        let mut terminate = signal::unix::signal(signal::unix::SignalKind::terminate())
            .context("failed to install SIGTERM handler")?;
        tokio::select! {
            result = signal::ctrl_c() => {
                result.context("failed to install Ctrl-C handler")?;
                info!("Ctrl-C received, stopping all TON processes");
            }
            _ = terminate.recv() => {
                info!("SIGTERM received, stopping all TON processes");
            }
        }
    }
    #[cfg(not(unix))]
    {
        signal::ctrl_c()
            .await
            .context("failed to install Ctrl-C handler")?;
        info!("Ctrl-C received, stopping all TON processes");
    }
    Ok(())
}

/// Accepts the two block-id formats emitted by supported lite-client builds.
fn parse_masterchain_seqno(output: &str) -> Result<u32> {
    let patterns = [
        Regex::new(r"(?i)seqno[=:\s]+(\d+)")?,
        Regex::new(r"\(-1,[^,\r\n]+,(\d+)\)")?,
    ];
    for regex in patterns {
        if let Some(captures) = regex.captures(output) {
            return captures[1].parse().context("invalid masterchain seqno");
        }
    }
    bail!("lite-client output contains no masterchain seqno")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_lite_client_block_ids() {
        assert_eq!(
            parse_masterchain_seqno(
                "latest masterchain block known to server is (-1,8000000000000000,17)"
            )
            .unwrap(),
            17
        );
        assert_eq!(parse_masterchain_seqno("seqno: 42").unwrap(), 42);
    }
}
