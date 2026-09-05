//! Process support for the localnet Docker runtime.

use super::{
    COMPOSE_DELETE_TIMEOUT, COMPOSE_STOP_TIMEOUT, COMPOSE_WAIT_TIMEOUT_SECONDS, DockerNetwork,
    DockerTarget, descriptor::docker_text,
};
use crate::Error;
use std::{ffi::OsStr, fs::OpenOptions, process::Stdio, time::Duration};
use tokio::{
    process::{Child, Command},
    time::timeout,
};

pub(crate) struct IsolatedPullTarget {
    docker_host: String,
    platform: String,
}

impl DockerNetwork {
    pub(crate) fn spawn_normal_pull(&self) -> Result<Child, Error> {
        let mut command = self.normal_pull_command();
        self.spawn_logged(
            &mut command,
            true,
            "pull the full TON network image with Docker",
        )
    }

    pub(crate) fn spawn_image_inspect(&self) -> Result<Child, Error> {
        let mut command = self.image_inspect_command();
        self.spawn_logged(
            &mut command,
            true,
            "inspect the full TON network image with Docker",
        )
    }

    pub(crate) fn spawn_isolated_pull(&self, target: &IsolatedPullTarget) -> Result<Child, Error> {
        let mut command = self
            .isolated_pull_command(target)
            .ok_or_else(|| Error::Internal {
                code: "environment_start_failed",
                message: "The isolated Docker pull is unavailable for a custom image".to_owned(),
            })?;
        self.spawn_logged(
            &mut command,
            false,
            "pull the public full TON network image with an isolated Docker configuration",
        )
    }

    pub(crate) fn spawn_compose_up(&self) -> Result<Child, Error> {
        let mut command = self.compose_command();
        command
            .arg("up")
            .arg("-d")
            .arg("--wait")
            .arg("--wait-timeout")
            .arg(COMPOSE_WAIT_TIMEOUT_SECONDS.to_string());
        self.spawn_logged(
            &mut command,
            false,
            "start the full TON network with Docker Compose",
        )
    }

    pub(super) fn spawn_logged(
        &self,
        command: &mut Command,
        truncate: bool,
        operation: &str,
    ) -> Result<Child, Error> {
        let stdout = OpenOptions::new()
            .create(true)
            .truncate(truncate)
            .append(!truncate)
            .write(true)
            .open(&self.startup_log_file)
            .map_err(|error| Error::Internal {
                code: "environment_start_failed",
                message: format!(
                    "Failed to open Docker startup log at {}: {error}",
                    self.startup_log_file.display()
                ),
            })?;
        let stderr = stdout.try_clone().map_err(|error| Error::Internal {
            code: "environment_start_failed",
            message: format!(
                "Failed to open Docker startup log at {}: {error}",
                self.startup_log_file.display()
            ),
        })?;
        command
            .stdin(Stdio::null())
            .stdout(Stdio::from(stdout))
            .stderr(Stdio::from(stderr))
            .kill_on_drop(true);
        command.spawn().map_err(|error| Error::Internal {
            code: "environment_start_failed",
            message: format!("Failed to {operation}: {error}"),
        })
    }

    pub(crate) async fn stop(&self) -> Result<(), Error> {
        self.run_compose(
            ["stop"],
            "stop",
            "environment_stop_failed",
            COMPOSE_STOP_TIMEOUT,
        )
        .await
    }

    pub(crate) async fn delete(&self) -> Result<(), Error> {
        self.run_compose(
            ["down", "--volumes", "--remove-orphans"],
            "delete",
            "environment_delete_failed",
            COMPOSE_DELETE_TIMEOUT,
        )
        .await
    }

    pub(crate) async fn isolated_pull_target(&self) -> Result<Option<IsolatedPullTarget>, Error> {
        if self.isolated_docker_config_dir.is_none() {
            return Ok(None);
        }

        let docker_host = match &self.docker_target {
            DockerTarget::Host(host) => host.clone(),
            DockerTarget::Context(context) => {
                self.docker_text([
                    "context",
                    "inspect",
                    "--format",
                    "{{.Endpoints.docker.Host}}",
                    context,
                ])
                .await?
            }
        };

        if !docker_host.starts_with("unix://") {
            return Err(Error::Internal {
                code: "environment_start_failed",
                message: format!(
                    "The isolated image pull is unsafe for Docker endpoint {docker_host}"
                ),
            });
        }

        let server_platform = self
            .docker_text(["version", "--format", "{{.Server.Os}}/{{.Server.Arch}}"])
            .await?;
        let platform = match server_platform.as_str() {
            "linux/arm64" | "linux/aarch64" => "linux/arm64",
            "linux/amd64" | "linux/x86_64" => "linux/amd64",
            _ => {
                return Err(Error::Internal {
                    code: "environment_start_failed",
                    message: format!(
                        "The isolated image pull does not support Docker platform {server_platform}"
                    ),
                });
            }
        };

        Ok(Some(IsolatedPullTarget {
            docker_host,
            platform: platform.to_owned(),
        }))
    }

    fn normal_pull_command(&self) -> Command {
        let mut command = self.docker_command();
        command.arg("pull").arg(&self.image);
        command
    }

    fn image_inspect_command(&self) -> Command {
        let mut command = self.docker_command();
        command
            .arg("image")
            .arg("inspect")
            .arg("--format")
            .arg("{{.Id}}")
            .arg(&self.image);
        command
    }

    fn isolated_pull_command(&self, target: &IsolatedPullTarget) -> Option<Command> {
        let config_dir = self.isolated_docker_config_dir.as_ref()?;
        let mut command = Command::new("docker");
        command
            .arg("--config")
            .arg(config_dir)
            .arg("--host")
            .arg(&target.docker_host)
            .arg("pull")
            .arg("--platform")
            .arg(&target.platform)
            .arg(&self.image);
        Some(command)
    }

    pub(super) fn compose_command(&self) -> Command {
        let mut command = self.docker_command();
        command
            .arg("compose")
            .arg("-p")
            .arg(&self.project_name)
            .arg("-f")
            .arg(&self.compose_file);
        command
    }

    pub(super) fn docker_command(&self) -> Command {
        let mut command = Command::new("docker");
        match &self.docker_target {
            DockerTarget::Context(context) => {
                command.arg("--context").arg(context);
            }
            DockerTarget::Host(host) => {
                command.arg("--host").arg(host);
            }
        }
        command
    }

    async fn docker_text<I, S>(&self, args: I) -> Result<String, Error>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        let mut command = self.docker_command();
        command.args(args);
        docker_text(command).await
    }

    pub(super) async fn run_command(
        &self,
        command: Command,
        operation: &str,
        code: &'static str,
        operation_timeout: Duration,
    ) -> Result<(), Error> {
        self.command_output(command, operation, code, operation_timeout)
            .await
            .map(|_| ())
    }

    pub(super) async fn command_output(
        &self,
        mut command: Command,
        operation: &str,
        code: &'static str,
        operation_timeout: Duration,
    ) -> Result<std::process::Output, Error> {
        command.stdin(Stdio::null()).kill_on_drop(true);
        let output = timeout(operation_timeout, command.output())
            .await
            .map_err(|_| Error::Internal {
                code,
                message: format!(
                    "Timed out after {} seconds while trying to {operation}",
                    operation_timeout.as_secs()
                ),
            })?
            .map_err(|error| Error::Internal {
                code,
                message: format!("Failed to {operation}: {error}"),
            })?;
        if output.status.success() {
            return Ok(output);
        }

        let stderr = String::from_utf8_lossy(&output.stderr);
        let details = stderr.trim();
        Err(Error::Internal {
            code,
            message: if details.is_empty() {
                format!("Could not {operation} ({})", output.status)
            } else {
                format!("Could not {operation}: {details}")
            },
        })
    }

    pub(super) async fn run_compose<I, S>(
        &self,
        args: I,
        operation: &str,
        code: &'static str,
        operation_timeout: Duration,
    ) -> Result<(), Error>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        let mut command = self.compose_command();
        command.args(args);
        self.run_command(
            command,
            &format!("{operation} the full TON network with Docker Compose"),
            code,
            operation_timeout,
        )
        .await
    }
}
