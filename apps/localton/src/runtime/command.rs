//! Execution of bounded one-shot child commands.
//!
//! [`run_checked`] captures stdout and stderr, enforces a timeout, kills the
//! child when its future is dropped, and returns an error for non-zero exit
//! status. TON console clients and build helpers use the returned text output.

use std::{
    process::Stdio,
    time::{Duration, Instant},
};

use anyhow::{Context, Result, bail};
use tokio::{process::Command, time::timeout};
use tracing::{debug, warn};

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
    let started = Instant::now();
    // The semantic adapter logs safe fields such as operation, node, and target.
    // Raw argv is deliberately excluded because Fift and console invocations can
    // contain signed messages, wallet payloads, or other operator secrets.
    debug!(%label, "running command");
    command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    let output = match timeout(max_duration, command.output()).await {
        Ok(result) => result.with_context(|| format!("failed to execute {label}"))?,
        Err(error) => {
            warn!(
                %label,
                duration_ms = started.elapsed().as_millis(),
                outcome = "timeout",
                "command timed out"
            );
            return Err(error)
                .with_context(|| format!("{label} timed out after {}s", max_duration.as_secs()));
        }
    };
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    if !output.status.success() {
        // Raw output stays in the returned diagnostic instead of becoming a
        // tracing field: tool output can contain signed payloads or key material.
        warn!(
            %label,
            duration_ms = started.elapsed().as_millis(),
            status = %output.status,
            outcome = "failed",
            "command failed"
        );
        bail!(
            "{label} failed with {}\nstdout:\n{}\nstderr:\n{}",
            output.status,
            stdout.trim(),
            stderr.trim()
        );
    }
    debug!(
        %label,
        duration_ms = started.elapsed().as_millis(),
        status = %output.status,
        outcome = "completed",
        "command completed"
    );
    Ok(CommandOutput { stdout, stderr })
}
