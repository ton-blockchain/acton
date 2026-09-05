//! Shared Acton process ownership for CLI and application integrations.

use crate::{Error, catalog::NetworkDirectory, client::Client};
use std::{path::PathBuf, process::Stdio, time::Duration};
use tokio::{
    process::{Child, Command},
    time::Instant,
};

/// Launches the same installed Acton executable with explicit project and catalog roots.
/// Applications own returned children and must finish graceful shutdown before dropping them.
#[derive(Clone)]
pub struct Launcher {
    pub executable: PathBuf,
    pub project_root: PathBuf,
    pub catalog_root: PathBuf,
}

impl Launcher {
    /// Observes a stopped service through the configured executable as well, so
    /// executable wrappers and their Docker context apply consistently to inspection.
    pub async fn inspect(&self, network: &NetworkDirectory) -> Result<crate::Network, Error> {
        let output = self
            .command("status", network)
            .output()
            .await
            .map_err(process_error)?;
        if !output.status.success() {
            return Err(Error::Internal {
                code: "inspection_failed",
                message: String::from_utf8_lossy(&output.stderr).into_owned(),
            });
        }
        serde_json::from_slice(&output.stdout)
            .map_err(|error| Error::invalid(format!("Invalid localnet status response: {error}")))
    }

    fn command(&self, action: &str, network: &NetworkDirectory) -> Command {
        let mut command = Command::new(&self.executable);
        command
            .arg("--project-root")
            .arg(&self.project_root)
            .args(["localnet", "--state-dir"])
            .arg(&self.catalog_root)
            .arg(action)
            .arg(&network.network.id)
            .arg("--json")
            .current_dir(&self.project_root)
            .stdin(Stdio::null());
        command
    }

    /// Stops a foreground owner even when its control service is still being
    /// discovered. Waiting for discovery avoids signalling the CLI before its
    /// graceful signal handler has been installed during startup.
    pub async fn shutdown_started(
        &self,
        network: &NetworkDirectory,
        child: &mut Child,
    ) -> Result<(), Error> {
        let deadline = Instant::now() + Duration::from_secs(20);
        loop {
            if let Ok(client) = Client::connect(&network.path).await {
                // Signal the foreground owner first so cancellation during
                // startup follows its normal success path, then confirm the service.
                terminate(child).await?;
                return client.shutdown().await;
            }
            if child.try_wait().map_err(process_error)?.is_some() {
                return Ok(());
            }
            if Instant::now() >= deadline {
                return terminate(child).await;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    }

    /// Starts a foreground owner using the public CLI command. HTTP discovery is
    /// published before Docker startup, so applications can immediately observe progress.
    pub fn start(&self, network: &NetworkDirectory) -> Result<Child, Error> {
        self.spawn("start", network)
    }

    fn spawn(&self, action: &str, network: &NetworkDirectory) -> Result<Child, Error> {
        let path = network.path.join(if action == "start" {
            "owner.log"
        } else {
            "service.log"
        });
        let log = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .map_err(|error| Error::storage(&path, error))?;
        let mut command = self.command(action, network);
        command
            .stdout(Stdio::from(log.try_clone().map_err(process_error)?))
            .stderr(Stdio::from(log));

        // Only the foreground owner handles terminal termination; its service
        // receives the authenticated shutdown request after startup is interrupted.
        #[cfg(unix)]
        command.process_group(0);

        command.spawn().map_err(process_error)
    }

    /// Opens the control API without starting Docker, for operations on stopped networks.
    /// The optional child marks ownership; a concurrent winner is only attached to.
    pub async fn connect_or_start(
        &self,
        location: NetworkDirectory,
    ) -> Result<(Client, Option<Child>), Error> {
        if let Ok(client) = Client::connect(&location.path).await {
            return Ok((client, None));
        }

        let location = location.prepare(&self.catalog_root).await?;
        let root = &location.path;
        let log_path = root.join("service.log");
        let mut child = self.spawn("serve", &location)?;
        let deadline = Instant::now() + Duration::from_secs(20);

        loop {
            if let Ok(client) = Client::connect(root).await {
                if child.id() == Some(client.service_pid()) {
                    return Ok((client, Some(child)));
                }

                // This child lost the lock. Reap it before adopting the winner, and
                // never make this command the owner of somebody else's service.
                let _ = child.wait().await;
                return Ok((client, None));
            }

            if let Some(status) = child.try_wait().map_err(process_error)? {
                // Concurrent callers may lose the filesystem lock to another service.
                // Only a verified descriptor permits adopting the winner.
                if let Ok(client) = Client::connect(root).await {
                    return Ok((client, None));
                }
                return Err(Error::Internal {
                    code: "service_exited",
                    message: format!(
                        "Localnet service exited with {status}; full log: {}",
                        log_path.display()
                    ),
                });
            }

            if Instant::now() >= deadline {
                terminate(&mut child).await?;
                return Err(Error::Internal {
                    code: "service_timeout",
                    message: format!(
                        "Localnet service did not become ready; full log: {}",
                        log_path.display()
                    ),
                });
            }

            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    }
}

/// Signals only a process obtained from a child handle, then waits for its cleanup.
/// SIGTERM follows the same graceful path as Ctrl-C; it never targets sibling networks.
pub async fn terminate(child: &mut Child) -> Result<(), Error> {
    if child.try_wait().map_err(process_error)?.is_some() {
        return Ok(());
    }
    if let Some(pid) = child.id() {
        #[cfg(unix)]
        {
            let status = Command::new("kill")
                .args(["-TERM", &pid.to_string()])
                .status()
                .await
                .map_err(process_error)?;
            if !status.success() && child.try_wait().map_err(process_error)?.is_none() {
                return Err(Error::Internal {
                    code: "signal_failed",
                    message: "Failed to signal the owned localnet process".to_owned(),
                });
            }
        }
        #[cfg(not(unix))]
        child.start_kill().map_err(process_error)?;
    }
    let status = child.wait().await.map_err(process_error)?;
    if !status.success() {
        return Err(Error::Internal {
            code: "shutdown_failed",
            message: format!("Localnet process exited with {status}; inspect its service.log"),
        });
    }
    Ok(())
}

fn process_error(error: std::io::Error) -> Error {
    Error::Internal {
        code: "process_failed",
        message: format!("Localnet process failed: {error}"),
    }
}
