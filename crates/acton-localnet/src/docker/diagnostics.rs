//! Diagnostics support for the localnet Docker runtime.

use super::{
    DOCKER_DIAGNOSTICS_TIMEOUT, DOCKER_METADATA_TIMEOUT, DockerNetwork, FAILED_CONTAINER_LOG_LINES,
    STARTUP_ERROR_LINES,
};
use crate::{Error, Node, ServiceHealth, ServiceHealthStatus};
use serde::Deserialize;
use std::{
    process::{ExitStatus, Stdio},
    time::Duration,
};
use tokio::{process::Command, time::timeout};

const CORE_SERVICES: [&str; 9] = [
    "localton",
    "postgres",
    "redis",
    "v3-basechain-bootstrap",
    "v3-migrations",
    "v3-worker",
    "v3-account-scanner",
    "v3-api",
    "v3-classifier",
];

const ONE_SHOT_SERVICES: [&str; 2] = ["v3-basechain-bootstrap", "v3-migrations"];

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct ComposeContainerState {
    #[serde(default)]
    name: String,
    #[serde(default)]
    service: String,
    #[serde(default)]
    state: String,
    #[serde(default)]
    health: String,
    #[serde(default)]
    exit_code: i32,
}

impl ComposeContainerState {
    fn failed(&self) -> bool {
        self.exit_code != 0
            || self.health.eq_ignore_ascii_case("unhealthy")
            || matches!(
                self.state.to_ascii_lowercase().as_str(),
                "dead" | "restarting"
            )
    }

    fn label(&self) -> String {
        let service = if self.service.is_empty() {
            self.name.as_str()
        } else {
            self.service.as_str()
        };

        let mut status = Vec::new();
        if !self.state.is_empty() {
            status.push(self.state.clone());
        }

        if !self.health.is_empty() {
            status.push(self.health.clone());
        }

        if self.exit_code != 0 {
            status.push(format!("exit code {}", self.exit_code));
        }

        if status.is_empty() {
            service.to_owned()
        } else {
            format!("{service} ({})", status.join(", "))
        }
    }

    fn normalized_status(&self, one_shot: bool) -> ServiceHealthStatus {
        if self.failed() {
            return ServiceHealthStatus::Failed;
        }

        if one_shot && self.state.eq_ignore_ascii_case("exited") {
            return ServiceHealthStatus::Completed;
        }

        if self.state.eq_ignore_ascii_case("running") {
            return if self.health.is_empty() || self.health.eq_ignore_ascii_case("healthy") {
                ServiceHealthStatus::Ready
            } else {
                ServiceHealthStatus::Starting
            };
        }

        if matches!(
            self.state.to_ascii_lowercase().as_str(),
            "created" | "starting"
        ) {
            return ServiceHealthStatus::Starting;
        }

        if self.state.eq_ignore_ascii_case("exited") {
            return ServiceHealthStatus::Stopped;
        }

        ServiceHealthStatus::Unknown
    }

    fn health(&self, one_shot: bool) -> ServiceHealth {
        ServiceHealth {
            name: self.service.clone(),
            status: self.normalized_status(one_shot),
            state: (!self.state.is_empty()).then(|| self.state.clone()),
            health: (!self.health.is_empty()).then(|| self.health.clone()),
            exit_code: Some(self.exit_code),
        }
    }
}

impl DockerNetwork {
    /// Reports readiness against the rendered topology, including one-shot jobs.
    /// A running container with a pending health check is not yet ready.
    pub(crate) async fn container_progress(
        &self,
        nodes: &[Node],
        stopping: bool,
    ) -> Option<crate::OperationProgress> {
        let mut command = self.compose_command();
        command.args(["ps", "--all", "--format", "json"]);
        let output = timeout(Duration::from_secs(2), diagnostic_output(command))
            .await
            .ok()??;
        if !output.status.success() {
            return None;
        }

        let states = parse_compose_container_states(&String::from_utf8_lossy(&output.stdout));
        let mut pending = Vec::new();
        let mut completed = 0;

        for service in CORE_SERVICES
            .into_iter()
            .chain(nodes.iter().map(|node| node.id.as_str()))
        {
            let state = states.iter().find(|state| state.service == service);
            let ready = if stopping {
                state.is_none_or(|state| matches!(state.state.as_str(), "exited" | "dead"))
            } else {
                state.is_some_and(|state| {
                    if ONE_SHOT_SERVICES.contains(&service) {
                        state.state == "exited" && state.exit_code == 0
                    } else {
                        state.state == "running"
                            && (state.health.is_empty() || state.health == "healthy")
                    }
                })
            };

            if ready {
                completed += 1;
            } else {
                // Exited containers may still carry the previous run's SIGTERM
                // status. It is not a failure of this startup; report what the
                // new run is waiting for and keep exit codes in error diagnostics.
                let waiting = if stopping {
                    "stopping"
                } else if state.is_some_and(|state| state.state == "running") {
                    if ONE_SHOT_SERVICES.contains(&service) {
                        "finishing"
                    } else {
                        "health check"
                    }
                } else {
                    "starting"
                };
                pending.push(format!("{service}: {waiting}"));
            }
        }

        Some(crate::OperationProgress {
            completed,
            total: Some(CORE_SERVICES.len() as u64 + nodes.len() as u64),
            unit: if stopping { "stopped" } else { "ready" }.to_owned(),
            detail: if pending.is_empty() {
                if stopping {
                    "All services stopped"
                } else {
                    "All services ready"
                }
                .to_owned()
            } else {
                let first = &pending[0];
                if pending.len() == 1 {
                    first.clone()
                } else {
                    format!("{first} (+{} waiting)", pending.len() - 1)
                }
            },
        })
    }

    /// Returns the current state of every Compose service in stable lifecycle order.
    /// Missing services remain visible as stopped so clients can explain an incomplete deployment.
    pub(crate) async fn service_health(&self, nodes: &[Node]) -> Result<Vec<ServiceHealth>, Error> {
        let mut command = self.compose_command();
        command.args(["ps", "--all", "--format", "json"]);
        let output = self
            .command_output(
                command,
                "inspect service health",
                "service_health_failed",
                DOCKER_METADATA_TIMEOUT,
            )
            .await?;
        let states = parse_compose_container_states(&String::from_utf8_lossy(&output.stdout));

        // Completed setup jobs lead the list because they explain whether the durable
        // index schema and starting boundary were prepared before live services ran.
        Ok(ONE_SHOT_SERVICES
            .into_iter()
            .chain(
                CORE_SERVICES
                    .into_iter()
                    .filter(|service| !ONE_SHOT_SERVICES.contains(service)),
            )
            .chain(nodes.iter().map(|node| node.id.as_str()))
            .map(|service| {
                states
                    .iter()
                    .find(|state| state.service == service)
                    .map_or_else(
                        || ServiceHealth {
                            name: service.to_owned(),
                            status: ServiceHealthStatus::Stopped,
                            state: None,
                            health: None,
                            exit_code: None,
                        },
                        |state| state.health(ONE_SHOT_SERVICES.contains(&service)),
                    )
            })
            .collect())
    }

    /// Classifies the Compose deployment while ignoring successful one-shot jobs.
    pub(crate) async fn status(&self) -> Result<crate::Status, Error> {
        let mut command = self.compose_command();
        command.args(["ps", "--all", "--format", "json"]);
        let output = self
            .command_output(
                command,
                "inspect network state",
                "status_failed",
                DOCKER_METADATA_TIMEOUT,
            )
            .await?;
        let states = parse_compose_container_states(&String::from_utf8_lossy(&output.stdout));
        if states.iter().all(|s| s.state != "running") {
            return Ok(crate::Status::Stopped);
        }

        let required = [
            "localton",
            "postgres",
            "redis",
            "v3-worker",
            "v3-api",
            "v3-classifier",
        ];
        if states.iter().any(ComposeContainerState::failed)
            || required.iter().any(|name| {
                !states
                    .iter()
                    .any(|s| s.service == *name && s.state == "running")
            })
        {
            return Ok(crate::Status::Failed);
        }

        Ok(crate::Status::Running)
    }

    pub(crate) async fn startup_failure_message(
        &self,
        operation: &str,
        status: ExitStatus,
    ) -> String {
        let mut message = format!("Docker exited with {status} while trying to {operation}");
        if let Ok(output) = tokio::fs::read_to_string(&self.startup_log_file).await {
            let lines = output
                .lines()
                .filter(|line| !line.trim().is_empty())
                .rev()
                .take(STARTUP_ERROR_LINES)
                .collect::<Vec<_>>();
            if !lines.is_empty() {
                let details = lines.into_iter().rev().collect::<Vec<_>>().join("\n");
                message.push_str(":\n");
                message.push_str(&details);
            }
        }

        if let Some(diagnostics) = self.failed_container_diagnostics().await {
            message.push_str("\n\nFailed container logs:\n");
            message.push_str(&diagnostics);
        }
        message
    }

    async fn failed_container_diagnostics(&self) -> Option<String> {
        let mut command = self.compose_command();
        command.args(["ps", "--all", "--format", "json"]);
        let output = diagnostic_output(command).await?;
        if !output.status.success() {
            return None;
        }

        let states = parse_compose_container_states(&String::from_utf8_lossy(&output.stdout));
        let failed = states
            .into_iter()
            .filter(ComposeContainerState::failed)
            .collect::<Vec<_>>();
        if failed.is_empty() {
            return None;
        }

        let mut diagnostics = Vec::with_capacity(failed.len());
        for container in failed {
            let mut section = container.label();
            let mut details = Vec::new();
            if !container.name.is_empty() {
                if container.health.eq_ignore_ascii_case("unhealthy") {
                    let mut command = self.docker_command();
                    command.args([
                        "inspect",
                        "--format",
                        "{{range .State.Health.Log}}{{println .Output}}{{end}}",
                        &container.name,
                    ]);
                    if let Some(output) = diagnostic_output(command).await {
                        let health_output = diagnostic_text(&output);
                        if !health_output.is_empty() {
                            details.push(format!("Health check output:\n{health_output}"));
                        }
                    }
                }

                let mut command = self.docker_command();
                command
                    .args(["logs", "--tail"])
                    .arg(FAILED_CONTAINER_LOG_LINES.to_string())
                    .arg(&container.name);
                if let Some(output) = diagnostic_output(command).await {
                    let logs = diagnostic_text(&output);
                    if !logs.is_empty() {
                        details.push(format!("Container logs:\n{logs}"));
                    }
                }
            }

            if !details.is_empty() {
                section.push_str(":\n");
                section.push_str(&details.join("\n\n"));
            }
            diagnostics.push(section);
        }
        Some(diagnostics.join("\n\n"))
    }
}

async fn diagnostic_output(mut command: Command) -> Option<std::process::Output> {
    command.stdin(Stdio::null()).kill_on_drop(true);
    timeout(DOCKER_DIAGNOSTICS_TIMEOUT, command.output())
        .await
        .ok()?
        .ok()
}

fn diagnostic_text(output: &std::process::Output) -> String {
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    [stdout.trim(), stderr.trim()]
        .into_iter()
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

fn parse_compose_container_states(output: &str) -> Vec<ComposeContainerState> {
    serde_json::from_str(output).unwrap_or_else(|_| {
        output
            .lines()
            .filter_map(|line| serde_json::from_str(line).ok())
            .collect()
    })
}
