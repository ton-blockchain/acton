//! Execution of bounded one-shot child commands.
//!
//! [`run_checked`] captures stdout and stderr, enforces a timeout, kills the
//! child when its future is dropped, and returns an error for non-zero exit
//! status. TON console clients and build helpers use the returned text output.

use std::{process::Stdio, time::Duration};

use anyhow::{Context, Result, bail};
use tokio::{process::Command, time::timeout};
use tracing::debug;

#[derive(Debug)]
pub struct CommandOutput {
    pub stdout: String,
    pub stderr: String,
}

pub async fn run_checked(
    label: &str,
    mut command: Command,
    max_duration: Duration,
) -> Result<CommandOutput> {
    debug!(%label, command = ?command.as_std(), "running command");
    command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    let output = timeout(max_duration, command.output())
        .await
        .with_context(|| format!("{label} timed out after {}s", max_duration.as_secs()))?
        .with_context(|| format!("failed to execute {label}"))?;
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    if !output.status.success() {
        bail!(
            "{label} failed with {}\nstdout:\n{}\nstderr:\n{}",
            output.status,
            stdout.trim(),
            stderr.trim()
        );
    }
    Ok(CommandOutput { stdout, stderr })
}
