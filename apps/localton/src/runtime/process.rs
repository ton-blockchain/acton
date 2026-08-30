//! Lifecycle of one long-running child process group.
//!
//! [`ManagedProcess::spawn`] redirects output to append-only log files and gives
//! the child its own process group. [`ManagedProcess::stop`] sends SIGTERM,
//! waits for normal exit, and escalates to SIGKILL. Dropping the value also
//! kills the complete group so descendants cannot become orphan processes.

use std::{
    fs::{self, OpenOptions},
    path::Path,
    process::{ExitStatus, Stdio},
    time::{Duration, Instant},
};

use anyhow::{Context, Result};
use async_trait::async_trait;
use nix::{
    sys::signal::{Signal, kill, killpg},
    unistd::Pid,
};
use tokio::{
    process::{Child, Command},
    time::{sleep, timeout},
};
use tracing::{info, warn};

use super::{ManagedService, ServiceExit, ServiceHandle};

/// Operating-system implementation of a long-running managed service.
///
/// Each child starts in its own process group so shutdown and drop can terminate
/// descendants as well as the direct child. The type intentionally owns the
/// Tokio child handle; transferring it into a [`ServiceHandle`] transfers the
/// responsibility for reaping and emergency cleanup with it.
pub struct ManagedProcess {
    name: String,
    child: Child,
    pid: Pid,
    started_at: Instant,
    exit_reported: bool,
}

impl Drop for ManagedProcess {
    fn drop(&mut self) {
        // The process group may still contain descendants after Tokio reaps
        // the direct child and Child::id() starts returning None.
        if self.child.id().is_some() {
            warn!(
                service = %self.name,
                pid = self.pid.as_raw(),
                lifetime_ms = self.started_at.elapsed().as_millis(),
                outcome = "forced_on_drop",
                "managed service dropped while still running"
            );
        }
        let _ = killpg(self.pid, Signal::SIGKILL);
        if self.child.id().is_some() {
            let _ = self.child.start_kill();
        }
    }
}

impl ManagedProcess {
    /// Starts a long-running child with isolated process-group ownership and
    /// append-only logs.
    ///
    /// Only the stable service name and resulting PID are logged. The executable,
    /// argv, and environment are deliberately omitted because TON invocations may
    /// refer to key material or other sensitive paths.
    pub fn spawn(
        name: impl Into<String>,
        mut command: Command,
        stdout_log: &Path,
        stderr_log: &Path,
    ) -> Result<Self> {
        let name = name.into();
        if let Some(parent) = stdout_log.parent() {
            fs::create_dir_all(parent)?;
        }
        let stdout = OpenOptions::new()
            .create(true)
            .append(true)
            .open(stdout_log)?;
        let stderr = OpenOptions::new()
            .create(true)
            .append(true)
            .open(stderr_log)?;
        command
            .stdin(Stdio::null())
            .stdout(Stdio::from(stdout))
            .stderr(Stdio::from(stderr))
            .kill_on_drop(true);
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            command.as_std_mut().process_group(0);
        }
        let child = command
            .spawn()
            .with_context(|| format!("failed to start {name}"))?;
        let pid = Pid::from_raw(
            child
                .id()
                .with_context(|| format!("{name} did not expose its pid"))?
                .try_into()
                .context("child pid does not fit pid_t")?,
        );
        info!(
            service = %name,
            pid = pid.as_raw(),
            outcome = "started",
            "managed service started"
        );
        Ok(Self {
            name,
            child,
            pid,
            started_at: Instant::now(),
            exit_reported: false,
        })
    }

    /// Returns the stable service name used in the process registry and logs.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the direct child's PID while Tokio still considers it running.
    ///
    /// The stored process-group ID remains available internally after this turns
    /// into `None`, which is why descendant cleanup does not depend on this value.
    pub fn id(&self) -> Option<u32> {
        self.child.id()
    }

    /// Checks for child exit without blocking and records the first observed
    /// outcome with its total lifetime.
    pub fn try_status(&mut self) -> Result<Option<ExitStatus>> {
        let status = self
            .child
            .try_wait()
            .with_context(|| format!("failed to inspect {}", self.name))?;
        if let Some(status) = status
            && !self.exit_reported
        {
            info!(
                service = %self.name,
                pid = self.pid.as_raw(),
                status = %status,
                success = status.success(),
                code = status.code(),
                lifetime_ms = self.started_at.elapsed().as_millis(),
                outcome = "exited",
                "managed service exit observed"
            );
            self.exit_reported = true;
        }
        Ok(status)
    }

    /// Stops the complete process group, escalating from SIGTERM to SIGKILL.
    ///
    /// The direct child receives up to five seconds for graceful termination.
    /// Descendants are cleaned up even after the leader exits, preventing helper
    /// processes from surviving instance shutdown. Cleanup is best-effort after
    /// SIGKILL so teardown is not permanently blocked by an uninterruptible child.
    pub async fn stop(&mut self) -> Result<()> {
        let stop_started = Instant::now();
        info!(
            service = %self.name,
            pid = self.pid.as_raw(),
            "stopping managed service"
        );
        let leader_exited = self.try_status()?.is_some();
        let group_signaled = match killpg(self.pid, Signal::SIGTERM) {
            Ok(()) => true,
            Err(error) => {
                if !leader_exited {
                    warn!(
                        service = %self.name,
                        pid = self.pid.as_raw(),
                        %error,
                        outcome = "group_sigterm_failed",
                        "failed to send SIGTERM to managed service group"
                    );
                    if let Err(error) = kill(self.pid, Signal::SIGTERM) {
                        warn!(
                            service = %self.name,
                            pid = self.pid.as_raw(),
                            %error,
                            outcome = "child_sigterm_failed",
                            "failed to send SIGTERM to managed service child"
                        );
                    }
                }
                false
            }
        };
        if leader_exited {
            if group_signaled {
                sleep(Duration::from_millis(500)).await;
                let _ = killpg(self.pid, Signal::SIGKILL);
            }
            info!(
                service = %self.name,
                pid = self.pid.as_raw(),
                duration_ms = stop_started.elapsed().as_millis(),
                outcome = "already_exited",
                "managed service stop completed"
            );
            return Ok(());
        }
        for _ in 0..50 {
            if self.try_status()?.is_some() {
                sleep(Duration::from_millis(500)).await;
                let _ = killpg(self.pid, Signal::SIGKILL);
                info!(
                    service = %self.name,
                    pid = self.pid.as_raw(),
                    duration_ms = stop_started.elapsed().as_millis(),
                    outcome = "graceful",
                    "managed service stop completed"
                );
                return Ok(());
            }
            sleep(Duration::from_millis(100)).await;
        }
        warn!(
            service = %self.name,
            pid = self.pid.as_raw(),
            duration_ms = stop_started.elapsed().as_millis(),
            outcome = "forcing_kill",
            "managed service did not stop after SIGTERM; sending SIGKILL"
        );
        if killpg(self.pid, Signal::SIGKILL).is_err() {
            let _ = kill(self.pid, Signal::SIGKILL);
        }
        let outcome = match timeout(Duration::from_secs(5), self.child.wait()).await {
            Ok(Ok(status)) => status.to_string(),
            Ok(Err(error)) => format!("wait failed: {error}"),
            Err(_) => "wait timed out".to_owned(),
        };
        info!(
            service = %self.name,
            pid = self.pid.as_raw(),
            duration_ms = stop_started.elapsed().as_millis(),
            outcome = "forced",
            result = %outcome,
            "managed service stop completed"
        );
        Ok(())
    }
}

#[async_trait]
impl ManagedService for ManagedProcess {
    fn name(&self) -> &str {
        ManagedProcess::name(self)
    }

    fn pid(&self) -> Option<u32> {
        ManagedProcess::id(self)
    }

    fn try_status(&mut self) -> Result<Option<ServiceExit>> {
        ManagedProcess::try_status(self).map(|status| status.map(ServiceExit::from))
    }

    async fn stop(&mut self) -> Result<()> {
        ManagedProcess::stop(self).await
    }
}

impl From<ManagedProcess> for ServiceHandle {
    fn from(process: ManagedProcess) -> Self {
        Self::new(process)
    }
}
