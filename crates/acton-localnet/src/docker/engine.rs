//! Engine operations own only resources bearing this deployment's labels.
//! Containers survive client drops; only explicit stop/delete operations affect them.

use super::{DockerNetwork, deployment::Condition, prerequisites::api_error};
use crate::Error;
use base64::Engine as _;
use bollard::{
    Docker,
    models::{
        ContainerStateStatusEnum, ContainerSummary, HealthStatusEnum, NetworkCreateRequest,
        VolumeCreateRequest,
    },
    query_parameters::{
        CreateContainerOptions, CreateImageOptions, ListContainersOptions, RemoveContainerOptions,
        RemoveVolumeOptions, StopContainerOptions,
    },
};
use futures::TryStreamExt;
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    time::{Duration, Instant},
};
use tokio::io::AsyncWriteExt;

pub(super) const PROJECT_LABEL: &str = "org.ton.acton.localnet.project";
pub(super) const SERVICE_LABEL: &str = "org.ton.acton.localnet.service";
const CONFIG_LABEL: &str = "org.ton.acton.localnet.config";

pub(super) const fn missing(error: &bollard::errors::Error) -> bool {
    matches!(
        error,
        bollard::errors::Error::DockerResponseServerError {
            status_code: 404,
            ..
        }
    )
}

impl DockerNetwork {
    pub(super) async fn client(&self) -> Result<&Docker, Error> {
        self.client
            .get_or_try_init(|| Box::pin(self.docker_target.connect()))
            .await
    }

    pub(super) fn container_name(&self, service: &str) -> String {
        format!("{}-{service}-1", self.project_name)
    }

    pub(super) fn labels(&self, service: Option<&str>) -> HashMap<String, String> {
        let mut labels = HashMap::from([(PROJECT_LABEL.into(), self.project_name.clone())]);
        if let Some(service) = service {
            labels.insert(SERVICE_LABEL.into(), service.to_owned());
        }
        labels
    }

    pub(super) fn filters(&self) -> HashMap<String, Vec<String>> {
        HashMap::from([(
            "label".into(),
            vec![format!("{PROJECT_LABEL}={}", self.project_name)],
        )])
    }

    pub(super) async fn containers(&self) -> Result<Vec<ContainerSummary>, Error> {
        self.client()
            .await?
            .list_containers(Some(ListContainersOptions {
                all: true,
                filters: Some(self.filters()),
                ..Default::default()
            }))
            .await
            .map_err(api_error)
    }

    pub(super) async fn log_line(&self, line: &str) -> Result<(), Error> {
        let mut log = tokio::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.startup_log_file)
            .await
            .map_err(|e| Error::storage(&self.startup_log_file, e))?;
        log.write_all(format!("{line}\n").as_bytes())
            .await
            .map_err(|e| Error::storage(&self.startup_log_file, e))
    }

    /// Bounds an operation and persists its outcome without serializing API payloads.
    pub(super) async fn operation<T>(
        &self,
        name: &str,
        code: &'static str,
        duration: Duration,
        work: impl Future<Output = Result<T, Error>>,
    ) -> Result<T, Error> {
        let started = Instant::now();
        tracing::info!(operation = name, target = %self.project_name, duration_ms = 0, outcome = "started");
        let result = tokio::time::timeout(duration, work)
            .await
            .unwrap_or_else(|_| {
                Err(Error::Internal {
                    code,
                    message: format!("{name} exceeded {} seconds", duration.as_secs()),
                })
            });
        let outcome = if result.is_ok() {
            "completed"
        } else {
            "failed"
        };
        tracing::info!(operation = name, target = %self.project_name, duration_ms = started.elapsed().as_millis() as u64, outcome);
        self.log_line(&format!(
            "operation={name} target={} duration_ms={} outcome={outcome}",
            self.project_name,
            started.elapsed().as_millis()
        ))
        .await?;
        if let Err(error) = &result {
            self.log_line(&error.to_string()).await?;
        }
        result.map_err(|error| Error::Internal {
            code,
            message: format!(
                "{name}: {error}\nFull log: {}",
                self.startup_log_file.display()
            ),
        })
    }

    pub(crate) async fn image_present(&self) -> Result<bool, Error> {
        match self.client().await?.inspect_image(&self.image).await {
            Ok(_) => Ok(true),
            Err(e) if missing(&e) => Ok(false),
            Err(e) => Err(api_error(e)),
        }
    }

    /// Pulls directly from the engine's event stream. Public Localton pulls never
    /// invoke host credential helpers; inline registry credentials support custom images.
    pub(crate) async fn pull_image(&self, image: &str) -> Result<(), Error> {
        let client = self.client().await?;
        let config = super::connection::configuration().await?;
        let first = image.split('/').next().unwrap_or_default();
        let registry = if image.contains('/')
            && (first.contains('.') || first.contains(':') || first == "localhost")
        {
            first
        } else {
            "https://index.docker.io/v1/"
        };
        let mut credentials = if image == super::DEFAULT_LOCALTON_IMAGE {
            None
        } else {
            config["auths"]
                .get(registry)
                .cloned()
                .map(serde_json::from_value::<bollard::auth::DockerCredentials>)
                .transpose()
                .map_err(|_| Error::invalid("Invalid Docker registry credentials"))?
        };
        if let Some(credentials) = &mut credentials {
            credentials.serveraddress = Some(registry.to_owned());
            if let Some(encoded) = credentials.auth.take() {
                let decoded = base64::engine::general_purpose::STANDARD
                    .decode(encoded)
                    .ok()
                    .and_then(|bytes| String::from_utf8(bytes).ok())
                    .ok_or_else(|| Error::invalid("Invalid Docker registry credentials"))?;
                let (username, password) = decoded
                    .split_once(':')
                    .ok_or_else(|| Error::invalid("Invalid Docker registry credentials"))?;
                credentials.username = Some(username.to_owned());
                credentials.password = Some(password.to_owned());
            }
        }
        let mut events = client.create_image(
            Some(CreateImageOptions {
                from_image: Some(image.into()),
                ..Default::default()
            }),
            None,
            credentials,
        );
        let mut layers = BTreeMap::new();
        *self.pull_progress.write().await = None;
        while let Some(event) = events.try_next().await.map_err(api_error)? {
            if let Some(message) = event.error_detail.and_then(|e| e.message) {
                return Err(api_error(
                    bollard::errors::Error::DockerResponseServerError {
                        status_code: 500,
                        message,
                    },
                ));
            }
            if let (Some(id), Some(status)) = (event.id, event.status) {
                layers.insert(
                    id.clone(),
                    matches!(status.as_str(), "Pull complete" | "Already exists"),
                );
                *self.pull_progress.write().await = Some(crate::OperationProgress {
                    completed: layers.values().filter(|ready| **ready).count() as u64,
                    total: None,
                    unit: "layers ready".into(),
                    detail: format!("{id}: {status}"),
                });
            }
        }
        Ok(())
    }

    pub(crate) async fn pull(&self) -> Result<(), Error> {
        self.operation(
            "pull_image",
            "image_pull_failed",
            Duration::from_secs(1800),
            self.pull_image(&self.image),
        )
        .await
    }

    pub(super) async fn ensure_image(&self, image: &str) -> Result<(), Error> {
        match self.client().await?.inspect_image(image).await {
            Ok(_) => Ok(()),
            Err(e) if missing(&e) => self.pull_image(image).await,
            Err(e) => Err(api_error(e)),
        }
    }

    pub(super) fn owned(&self, labels: Option<&HashMap<String, String>>) -> Result<(), Error> {
        if labels.and_then(|labels| labels.get(PROJECT_LABEL)) == Some(&self.project_name) {
            Ok(())
        } else {
            Err(Error::invalid(
                "A Docker resource with this name belongs to another deployment",
            ))
        }
    }

    async fn ensure_resources(
        &self,
        volumes: &BTreeMap<String, Option<serde_json::Value>>,
    ) -> Result<(), Error> {
        let client = self.client().await?;
        let network = format!("{}_default", self.project_name);
        match client.inspect_network(&network, None).await {
            Ok(existing) => self.owned(existing.labels.as_ref())?,
            Err(e) if missing(&e) => {
                client
                    .create_network(NetworkCreateRequest {
                        name: network,
                        driver: Some("bridge".into()),
                        labels: Some(self.labels(None)),
                        ..Default::default()
                    })
                    .await
                    .map_err(api_error)?;
            }
            Err(e) => return Err(api_error(e)),
        }
        for volume in volumes.keys() {
            self.ensure_volume(volume).await?;
        }
        Ok(())
    }

    pub(super) async fn ensure_volume(&self, volume: &str) -> Result<(), Error> {
        let client = self.client().await?;
        let name = format!("{}_{volume}", self.project_name);
        match client.inspect_volume(&name).await {
            Ok(existing) => self.owned(Some(&existing.labels))?,
            Err(e) if missing(&e) => {
                client
                    .create_volume(VolumeCreateRequest {
                        name: Some(name),
                        labels: Some(self.labels(None)),
                        ..Default::default()
                    })
                    .await
                    .map_err(api_error)?;
            }
            Err(e) => return Err(api_error(e)),
        }
        Ok(())
    }

    /// Starts only selected services (or the enabled topology). Successful one-shot
    /// jobs are reused until their configuration or persistent index is replaced.
    pub(super) fn start_services<'a>(
        &'a self,
        selected: Option<&'a [String]>,
        dependencies: bool,
        wait: bool,
    ) -> futures::future::BoxFuture<'a, Result<(), Error>> {
        Box::pin(async move {
            let deployment = self.deployment().await?;
            let client = self.client().await?;
            let mut pending: BTreeSet<String> = match selected {
                Some(selected) => selected.iter().cloned().collect(),
                None => deployment
                    .services
                    .iter()
                    .filter(|(_, s)| s.profiles.is_empty())
                    .map(|(n, _)| n.clone())
                    .collect(),
            };
            if dependencies {
                loop {
                    let before = pending.len();
                    for name in pending.clone() {
                        let service = deployment
                            .services
                            .get(&name)
                            .ok_or_else(|| Error::invalid("Unknown Localton service"))?;
                        pending.extend(service.depends_on.keys().cloned());
                    }
                    if pending.len() == before {
                        break;
                    }
                }
            }
            // Validate the dependency graph before creating any resources.
            let mut unchecked = pending.clone();
            while !unchecked.is_empty() {
                let ready: Vec<_> = unchecked
                    .iter()
                    .filter(|name| {
                        deployment.services.get(*name).is_some_and(|service| {
                            !dependencies
                                || service
                                    .depends_on
                                    .keys()
                                    .all(|dep| !unchecked.contains(dep))
                        })
                    })
                    .cloned()
                    .collect();
                if ready.is_empty() {
                    return Err(Error::invalid(
                        "Cyclic or unknown Localton service dependencies",
                    ));
                }
                for name in ready {
                    unchecked.remove(&name);
                }
            }
            self.ensure_resources(&deployment.volumes).await?;
            let mut started = BTreeSet::new();
            let mut changed = BTreeSet::new();
            let mut ready = BTreeSet::new();
            loop {
                for name in &pending {
                    let service = &deployment.services[name];
                    if !started.contains(name) {
                        let mut satisfied = true;
                        if dependencies {
                            for (dependency, requirement) in &service.depends_on {
                                if !started.contains(dependency)
                                    || !self.ready(dependency, requirement.condition).await?
                                {
                                    satisfied = false;
                                    break;
                                }
                            }
                        }
                        if !satisfied {
                            continue;
                        }
                        self.ensure_image(&service.image).await?;
                        let mut config = self.container_config(name, service).await?;
                        let mut definition = serde_json::to_value(&config)
                            .map_err(|e| Error::invalid(e.to_string()))?;
                        definition.sort_all_objects();
                        let hash = format!(
                            "{:x}",
                            Sha256::digest(
                                serde_json::to_vec(&definition)
                                    .map_err(|e| Error::invalid(e.to_string()))?
                            )
                        );
                        let container = self.container_name(name);
                        let mut create = true;
                        match client.inspect_container(&container, None).await {
                            Ok(existing) => {
                                let labels =
                                    existing.config.as_ref().and_then(|c| c.labels.as_ref());
                                self.owned(labels)?;
                                if labels.and_then(|l| l.get(CONFIG_LABEL)) == Some(&hash) {
                                    create = false;
                                } else {
                                    self.remove_service(name).await?;
                                }
                            }
                            Err(e) if missing(&e) => {}
                            Err(e) => return Err(api_error(e)),
                        }
                        if create {
                            config
                                .labels
                                .get_or_insert_default()
                                .insert(CONFIG_LABEL.into(), hash);
                            client
                                .create_container(
                                    Some(CreateContainerOptions {
                                        name: Some(container.clone()),
                                        ..Default::default()
                                    }),
                                    config,
                                )
                                .await
                                .map_err(api_error)?;
                        }
                        // A running peer can retain the old network namespace
                        // after its owner restarts, even when the owner's ID is unchanged.
                        if service.depends_on.iter().any(|(dependency, requirement)| {
                            requirement.restart && changed.contains(dependency)
                        }) {
                            self.stop_service(name).await?;
                        }
                        let state = client
                            .inspect_container(&container, None)
                            .await
                            .map_err(api_error)?
                            .state
                            .unwrap_or_default();
                        let one_shot =
                            matches!(name.as_str(), "v3-migrations" | "v3-basechain-bootstrap");
                        let completed = one_shot
                            && state.status == Some(ContainerStateStatusEnum::EXITED)
                            && state.exit_code == Some(0);
                        if state.running != Some(true) && !completed {
                            client
                                .start_container(&container, None)
                                .await
                                .map_err(api_error)?;
                            changed.insert(name.clone());
                        }
                        self.log_line(&format!(
                            "operation=start_service node={name} outcome=started"
                        ))
                        .await?;
                        started.insert(name.clone());
                    }
                    let condition =
                        if matches!(name.as_str(), "v3-migrations" | "v3-basechain-bootstrap") {
                            Condition::CompletedSuccessfully
                        } else {
                            Condition::Healthy
                        };
                    if !wait || self.ready(name, condition).await? {
                        ready.insert(name.clone());
                    }
                }
                if ready.len() == pending.len() {
                    return Ok(());
                }
                tokio::time::sleep(Duration::from_millis(250)).await;
            }
        })
    }

    async fn ready(&self, service: &str, condition: Condition) -> Result<bool, Error> {
        let state = self
            .client()
            .await?
            .inspect_container(&self.container_name(service), None)
            .await
            .map_err(api_error)?
            .state
            .unwrap_or_default();
        if state.dead == Some(true)
            || state.health.as_ref().and_then(|h| h.status) == Some(HealthStatusEnum::UNHEALTHY)
            || (state.status == Some(ContainerStateStatusEnum::EXITED)
                && state.exit_code != Some(0))
        {
            return Err(Error::Internal {
                code: "network_start_failed",
                message: format!("Service {service} failed (exit code {:?})", state.exit_code),
            });
        }
        Ok(match condition {
            Condition::Started => state.running == Some(true),
            Condition::CompletedSuccessfully => {
                state.status == Some(ContainerStateStatusEnum::EXITED) && state.exit_code == Some(0)
            }
            Condition::Healthy => {
                state.running == Some(true)
                    && state
                        .health
                        .as_ref()
                        .is_none_or(|h| h.status == Some(HealthStatusEnum::HEALTHY))
            }
        })
    }

    pub(super) async fn stop_service(&self, service: &str) -> Result<(), Error> {
        let client = self.client().await?;
        let name = self.container_name(service);
        match client.inspect_container(&name, None).await {
            Ok(container) => {
                self.owned(container.config.as_ref().and_then(|c| c.labels.as_ref()))?;
                if container.state.is_some_and(|s| s.running == Some(true)) {
                    client
                        .stop_container(
                            &name,
                            Some(StopContainerOptions {
                                t: Some(60),
                                ..Default::default()
                            }),
                        )
                        .await
                        .map_err(api_error)?;
                }
                Ok(())
            }
            Err(e) if missing(&e) => Ok(()),
            Err(e) => Err(api_error(e)),
        }
    }

    pub(super) async fn remove_service(&self, service: &str) -> Result<(), Error> {
        if service == "localton" {
            // Joined nodes refer to the owner's container ID. Remove their stale
            // containers before replacing the namespace; their volumes survive.
            for container in self.containers().await? {
                if let Some(node) = container.labels.as_ref().and_then(|l| l.get(SERVICE_LABEL))
                    && node.starts_with("node-")
                {
                    self.stop_service(node).await?;
                    if let Some(id) = container.id {
                        self.client()
                            .await?
                            .remove_container(
                                &id,
                                Some(RemoveContainerOptions {
                                    force: true,
                                    ..Default::default()
                                }),
                            )
                            .await
                            .map_err(api_error)?;
                    }
                }
            }
        }
        self.stop_service(service).await?;
        match self
            .client()
            .await?
            .remove_container(
                &self.container_name(service),
                Some(RemoveContainerOptions {
                    force: true,
                    ..Default::default()
                }),
            )
            .await
        {
            Ok(()) => Ok(()),
            Err(e) if missing(&e) => Ok(()),
            Err(e) => Err(api_error(e)),
        }
    }

    pub(super) async fn remove_volume(&self, volume: &str) -> Result<(), Error> {
        let client = self.client().await?;
        let name = format!("{}_{volume}", self.project_name);
        match client.inspect_volume(&name).await {
            Ok(volume) => self.owned(Some(&volume.labels))?,
            Err(e) if missing(&e) => return Ok(()),
            Err(e) => return Err(api_error(e)),
        }
        client
            .remove_volume(&name, None::<RemoveVolumeOptions>)
            .await
            .map_err(api_error)
    }

    /// Removes containers and their network while preserving chain/snapshot volumes.
    pub(super) async fn down(&self) -> Result<(), Error> {
        self.stop().await?;
        let client = self.client().await?;
        let mut containers = self.containers().await?;
        // Shared network namespaces must be released before their owner is removed.
        containers.sort_by_key(|c| {
            c.labels
                .as_ref()
                .and_then(|l| l.get(SERVICE_LABEL))
                .is_some_and(|s| s == "localton")
        });
        for container in containers {
            if let Some(id) = container.id {
                client
                    .remove_container(
                        &id,
                        Some(RemoveContainerOptions {
                            force: true,
                            ..Default::default()
                        }),
                    )
                    .await
                    .map_err(api_error)?;
            }
        }
        let name = format!("{}_default", self.project_name);
        match client.inspect_network(&name, None).await {
            Ok(network) => {
                self.owned(network.labels.as_ref())?;
                client.remove_network(&name).await.map_err(api_error)?;
            }
            Err(e) if missing(&e) => {}
            Err(e) => return Err(api_error(e)),
        }
        Ok(())
    }
}
