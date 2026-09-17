//! Diagnostics support for the localnet Docker runtime.

use super::{
    DOCKER_DIAGNOSTICS_TIMEOUT, DOCKER_METADATA_TIMEOUT, DockerNetwork, FAILED_CONTAINER_LOG_LINES,
    STARTUP_ERROR_LINES,
};
use crate::{DockerContainer, Error, Node, ServiceHealth, ServiceHealthStatus};
use bollard::query_parameters::LogsOptions;
use futures::TryStreamExt;
use std::{fmt::Write as _, time::Duration};
use tokio::time::timeout;

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

#[derive(Debug)]
struct ContainerState {
    id: String,
    name: String,
    image: String,
    service: String,
    state: String,
    health: String,
    exit_code: i32,
}

impl ContainerState {
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
            container: Some(DockerContainer {
                id: self.id.clone(),
                name: self.name.clone(),
                image: self.image.clone(),
            }),
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
        let states = timeout(Duration::from_secs(2), self.container_states())
            .await
            .ok()?
            .ok()?;
        let mut pending = Vec::new();
        let mut completed = 0;

        for service in CORE_SERVICES
            .into_iter()
            .chain(nodes.iter().map(|node| node.id.as_str()))
        {
            let state = states.iter().find(|state| state.service == service);
            let explicitly_stopped = nodes.iter().any(|node| node.id == service && node.stopped);
            let ready = if stopping || explicitly_stopped {
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

    /// Returns the current state of every managed service in stable lifecycle order.
    /// Missing services remain visible as stopped so clients can explain an incomplete deployment.
    pub(crate) async fn service_health(&self, nodes: &[Node]) -> Result<Vec<ServiceHealth>, Error> {
        let states = self.container_states().await?;

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
                            container: None,
                        },
                        |state| {
                            let mut health = state.health(ONE_SHOT_SERVICES.contains(&service));
                            if nodes.iter().any(|node| node.id == service && node.stopped)
                                && matches!(state.state.as_str(), "exited" | "dead" | "created")
                            {
                                health.status = ServiceHealthStatus::Stopped;
                            }
                            health
                        },
                    )
            })
            .collect())
    }

    /// Classifies the deployment while ignoring successful one-shot jobs.
    pub(crate) async fn status(&self, nodes: &[Node]) -> Result<crate::Status, Error> {
        let states = self.container_states().await?;
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
        if states.iter().any(|state| {
            state.failed()
                && !nodes.iter().any(|node| {
                    node.id == state.service
                        && node.stopped
                        && matches!(state.state.as_str(), "exited" | "dead" | "created")
                })
        }) || required.iter().any(|name| {
            !states
                .iter()
                .any(|s| s.service == *name && s.state == "running")
        }) {
            return Ok(crate::Status::Failed);
        }

        Ok(crate::Status::Running)
    }

    pub(crate) async fn startup_failure_message(&self, operation: &str, error: &Error) -> String {
        let mut message = format!("Docker failed to {operation}: {error}");
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

        if let Some(error) = super::prerequisites::runtime_failure(&message) {
            return error.to_string();
        }

        if let Some(diagnostics) = self.failed_container_diagnostics().await {
            message.push_str("\n\nFailed container logs:\n");
            message.push_str(&diagnostics);
        }
        format!(
            "Docker could not {operation}\nInspect the diagnostic details and full log, resolve the reported cause, then retry\nFull log: {}\n\n{message}",
            self.startup_log_file.display()
        )
    }

    /// Reads structured state, including exit codes which list-containers omits.
    /// An inspect racing with deletion is treated as an absent service.
    async fn container_states(&self) -> Result<Vec<ContainerState>, Error> {
        timeout(DOCKER_METADATA_TIMEOUT, async {
            let client = self.client().await?;
            let mut states = Vec::new();
            for container in self.containers().await? {
                let Some(service) = container
                    .labels
                    .as_ref()
                    .and_then(|l| l.get(super::engine::SERVICE_LABEL))
                else {
                    continue;
                };
                let Some(id) = container.id else {
                    continue;
                };
                let inspected = match client.inspect_container(&id, None).await {
                    Ok(inspected) => inspected,
                    Err(error) if super::engine::missing(&error) => continue,
                    Err(error) => return Err(super::prerequisites::api_error(error)),
                };
                let state = inspected.state.unwrap_or_default();
                states.push(ContainerState {
                    id,
                    service: service.clone(),
                    name: inspected
                        .name
                        .unwrap_or_default()
                        .trim_start_matches('/')
                        .to_owned(),
                    image: inspected.config.and_then(|c| c.image).unwrap_or_default(),
                    state: state.status.map(|s| s.to_string()).unwrap_or_default(),
                    health: state
                        .health
                        .and_then(|h| h.status)
                        .map(|s| s.to_string())
                        .unwrap_or_default(),
                    exit_code: state.exit_code.unwrap_or_default().try_into().unwrap_or(-1),
                });
            }
            Ok(states)
        })
        .await
        .map_err(|_| Error::Internal {
            code: "docker_check_failed",
            message: "Docker state query exceeded 10 seconds".into(),
        })?
    }

    pub(super) async fn failed_container_diagnostics(&self) -> Option<String> {
        timeout(DOCKER_DIAGNOSTICS_TIMEOUT, async {
            let states = self.container_states().await.ok()?;
            let client = self.client().await.ok()?;
            let mut diagnostics = Vec::new();
            for container in states.into_iter().filter(ContainerState::failed) {
                let mut section = container.label();
                if let Ok(inspected) = client.inspect_container(&container.id, None).await {
                    for entry in inspected
                        .state
                        .and_then(|s| s.health)
                        .and_then(|h| h.log)
                        .unwrap_or_default()
                    {
                        if let Some(output) = entry.output {
                            let _ = write!(section, "\nHealth check: {output}");
                        }
                    }
                }
                let mut logs = client.logs(
                    &container.id,
                    Some(LogsOptions {
                        stdout: true,
                        stderr: true,
                        tail: FAILED_CONTAINER_LOG_LINES.to_string(),
                        ..Default::default()
                    }),
                );
                let mut bytes = Vec::new();
                while let Ok(Some(frame)) = logs.try_next().await {
                    let remaining = (256 * 1024usize).saturating_sub(bytes.len());
                    bytes.extend_from_slice(&frame.as_ref()[..frame.as_ref().len().min(remaining)]);
                    if bytes.len() >= 256 * 1024 {
                        break;
                    }
                }
                if !bytes.is_empty() {
                    let _ = write!(
                        section,
                        "\nContainer logs:\n{}",
                        String::from_utf8_lossy(&bytes)
                    );
                }
                diagnostics.push(section);
            }
            let diagnostics = diagnostics.join("\n\n");
            if diagnostics.is_empty() {
                None
            } else {
                let _ = self.log_line(&diagnostics).await;
                Some(diagnostics)
            }
        })
        .await
        .ok()
        .flatten()
    }
}
