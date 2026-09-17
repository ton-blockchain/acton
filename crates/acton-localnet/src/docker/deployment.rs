//! The supported deployment schema is deliberately limited to our bundled templates.
//! Unknown fields fail before mutation, rather than silently losing Compose semantics.

use super::{DockerNetwork, prerequisites::api_error};
use crate::Error;
use bollard::models::ContainerCreateBody;
use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::BTreeMap;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Deployment {
    pub services: BTreeMap<String, Service>,
    pub volumes: BTreeMap<String, Option<Value>>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Service {
    pub image: String,
    pub command: Vec<String>,
    #[serde(default)]
    pub profiles: Vec<String>,
    #[serde(default)]
    pub depends_on: BTreeMap<String, Dependency>,
    #[serde(default)]
    environment: BTreeMap<String, String>,
    #[serde(default)]
    ports: Vec<String>,
    #[serde(default)]
    volumes: Vec<String>,
    #[serde(default)]
    security_opt: Vec<String>,
    network_mode: Option<String>,
    restart: Option<String>,
    stop_grace_period: Option<String>,
    user: Option<String>,
    shm_size: Option<String>,
    #[serde(default)]
    ulimits: BTreeMap<String, Limit>,
    healthcheck: Healthcheck,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Dependency {
    pub condition: Condition,
    #[serde(default)]
    pub restart: bool,
}

#[derive(Clone, Copy, Deserialize)]
pub(super) enum Condition {
    #[serde(rename = "service_started")]
    Started,
    #[serde(rename = "service_healthy")]
    Healthy,
    #[serde(rename = "service_completed_successfully")]
    CompletedSuccessfully,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Limit {
    soft: i64,
    hard: i64,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Healthcheck {
    #[serde(default)]
    disable: bool,
    test: Option<Vec<String>>,
    interval: Option<String>,
    timeout: Option<String>,
    start_period: Option<String>,
    start_interval: Option<String>,
    retries: Option<i64>,
}

fn invalid(message: impl std::fmt::Display) -> Error {
    Error::Internal {
        code: "environment_definition_invalid",
        message: format!("Invalid Localton deployment definition: {message}"),
    }
}

fn seconds(value: &str) -> Result<i64, Error> {
    value
        .strip_suffix('s')
        .ok_or_else(|| invalid("duration must use seconds"))?
        .parse()
        .map_err(invalid)
}

impl DockerNetwork {
    pub(super) async fn deployment(&self) -> Result<Deployment, Error> {
        let bytes = tokio::fs::read(&self.compose_file)
            .await
            .map_err(|e| Error::storage(&self.compose_file, e))?;
        let deployment: Deployment = serde_yaml_ng::from_slice(&bytes).map_err(invalid)?;
        for volume in deployment.volumes.values().flatten() {
            if !volume.as_object().is_some_and(serde_json::Map::is_empty) {
                return Err(invalid(
                    "only managed volumes without additional options are supported",
                ));
            }
        }
        for (name, service) in &deployment.services {
            for dependency in service.depends_on.keys() {
                if !deployment.services.contains_key(dependency) {
                    return Err(invalid(format!("{name} depends on missing {dependency}")));
                }
            }
        }
        Ok(deployment)
    }

    /// Translates the private service schema to an Engine request. Docker owns
    /// health probes and restart policies; Acton owns dependency ordering.
    pub(super) async fn container_config(
        &self,
        name: &str,
        service: &Service,
    ) -> Result<ContainerCreateBody, Error> {
        let mut bindings = serde_json::Map::new();
        let mut exposed = serde_json::Map::new();
        for port in &service.ports {
            let parts: Vec<_> = port.split(':').collect();
            let [host, published, target] = parts.as_slice() else {
                return Err(invalid("published ports require an explicit host address"));
            };
            let target = format!("{target}/tcp");
            bindings.insert(
                target.clone(),
                json!([{"HostIp": host, "HostPort": published}]),
            );
            exposed.insert(target, json!({}));
        }
        let binds = service
            .volumes
            .iter()
            .map(|volume| format!("{}_{volume}", self.project_name))
            .collect::<Vec<_>>();
        let network = format!("{}_default", self.project_name);
        let (mode, networking) = match &service.network_mode {
            Some(mode) => {
                let parent = mode
                    .strip_prefix("service:")
                    .ok_or_else(|| invalid("unsupported network mode"))?;
                // Pin the actual container ID: joined nodes share the owner's
                // namespace and must be recreated if that owner is recreated.
                let owner = self
                    .client()
                    .await?
                    .inspect_container(&self.container_name(parent), None)
                    .await
                    .map_err(api_error)?;
                let id = owner.id.ok_or_else(|| invalid("network owner has no ID"))?;
                (format!("container:{id}"), Value::Null)
            }
            None => (
                network.clone(),
                json!({"EndpointsConfig": {network: {"Aliases": [name]}}}),
            ),
        };
        let duration = |value: &Option<String>| -> Result<Option<i64>, Error> {
            value
                .as_deref()
                .map(seconds)
                .transpose()?
                .map(|s| {
                    s.checked_mul(1_000_000_000)
                        .ok_or_else(|| invalid("healthcheck duration overflow"))
                })
                .transpose()
        };
        let health = &service.healthcheck;
        let shm_size = service
            .shm_size
            .as_deref()
            .map(|size| -> Result<i64, Error> {
                let size: i64 = size
                    .strip_suffix('m')
                    .ok_or_else(|| invalid("shm_size must use megabytes"))?
                    .parse()
                    .map_err(invalid)?;
                size.checked_mul(1024 * 1024)
                    .ok_or_else(|| invalid("shm_size overflow"))
            })
            .transpose()?;
        serde_json::from_value(json!({
            "Image": service.image,
            "Cmd": service.command,
            "User": service.user,
            "Env": service.environment.iter().map(|(k,v)| format!("{k}={v}")).collect::<Vec<_>>(),
            "ExposedPorts": exposed,
            "StopTimeout": service.stop_grace_period.as_deref().map(seconds).transpose()?,
            "Labels": self.labels(Some(name)),
            "Healthcheck": if health.disable { json!({"Test": ["NONE"]}) } else { json!({
                "Test": health.test, "Interval": duration(&health.interval)?,
                "Timeout": duration(&health.timeout)?, "StartPeriod": duration(&health.start_period)?,
                "StartInterval": duration(&health.start_interval)?, "Retries": health.retries,
            }) },
            "HostConfig": {
                "Binds": binds, "PortBindings": bindings, "NetworkMode": mode,
                "SecurityOpt": service.security_opt, "ShmSize": shm_size,
                "RestartPolicy": {"Name": service.restart.as_deref().unwrap_or("no")},
                "Ulimits": service.ulimits.iter().map(|(name, l)| json!({"Name":name,"Soft":l.soft,"Hard":l.hard})).collect::<Vec<_>>(),
            },
            "NetworkingConfig": networking,
        })).map_err(invalid)
    }
}
