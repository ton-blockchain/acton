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
    time::Duration,
};

use anyhow::{Context, Result};
use nix::{
    sys::signal::{Signal, kill, killpg},
    unistd::Pid,
};
use tokio::{
    process::{Child, Command},
    time::{sleep, timeout},
};
use tracing::warn;

pub struct ManagedProcess {
    name: String,
    child: Child,
    pid: Pid,
}

impl Drop for ManagedProcess {
    fn drop(&mut self) {
        // The process group may still contain descendants after Tokio reaps
        // the direct child and Child::id() starts returning None.
        let _ = killpg(self.pid, Signal::SIGKILL);
        if self.child.id().is_some() {
            let _ = self.child.start_kill();
        }
    }
}

impl ManagedProcess {
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
        Ok(Self { name, child, pid })
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn id(&self) -> Option<u32> {
        self.child.id()
    }

    pub fn try_status(&mut self) -> Result<Option<ExitStatus>> {
        self.child
            .try_wait()
            .with_context(|| format!("failed to inspect {}", self.name))
    }

    pub async fn stop(&mut self) -> Result<()> {
        let leader_exited = self.try_status()?.is_some();
        let group_signaled = match killpg(self.pid, Signal::SIGTERM) {
            Ok(()) => true,
            Err(error) => {
                if !leader_exited {
                    warn!(process = %self.name, %error, "failed to send SIGTERM");
                    if let Err(error) = kill(self.pid, Signal::SIGTERM) {
                        warn!(process = %self.name, %error, "failed to send SIGTERM to child");
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
            return Ok(());
        }
        for _ in 0..50 {
            if self.try_status()?.is_some() {
                sleep(Duration::from_millis(500)).await;
                let _ = killpg(self.pid, Signal::SIGKILL);
                return Ok(());
            }
            sleep(Duration::from_millis(100)).await;
        }
        warn!(process = %self.name, "process did not stop; sending SIGKILL");
        if killpg(self.pid, Signal::SIGKILL).is_err() {
            let _ = kill(self.pid, Signal::SIGKILL);
        }
        let _ = timeout(Duration::from_secs(5), self.child.wait()).await;
        Ok(())
    }
}
