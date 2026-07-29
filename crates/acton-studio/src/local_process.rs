use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr, SocketAddr, TcpListener as StdTcpListener};
use std::path::{Path, PathBuf};
use std::process::{ExitStatus, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::Duration;

use tokio::net::TcpStream;
use tokio::process::{Child, Command};
use tokio::sync::{Mutex, Notify, RwLock};
use tokio::time::{Instant, sleep, timeout};

use crate::contract_registry::{ContractRegistryStore, VerifiedSourceRegistration};
use crate::environment::{
    CreateEnvironmentConfig, CreateEnvironmentRequest, EnvironmentConfig, EnvironmentEndpoints,
    EnvironmentRuntime, EnvironmentRuntimeError, EnvironmentRuntimeFuture, EnvironmentStatus,
    StudioEnvironment, UpdateEnvironmentRequest,
};
use crate::environment_store::{
    LoadedEnvironments, StoredEnvironment, load_environments, persist_environment,
};
use crate::full_ton_network::FullTonNetworkDriver;
use crate::local_artifacts::{ProjectArtifactSynchronizer, ProjectFingerprint};

const FIRST_LOCALNET_PORT: u16 = 5411;
const FIRST_FULL_TON_V2_PORT: u16 = 18080;
const FIRST_FULL_TON_V3_PORT: u16 = 18081;
const DEFAULT_FULL_TON_VALIDATORS: u16 = 1;
const MAX_FULL_TON_VALIDATORS: u16 = 100;
const LOCALNET_READY_TIMEOUT: Duration = Duration::from_secs(15);
const LOCALNET_READY_POLL_INTERVAL: Duration = Duration::from_millis(100);
const LOCALNET_STATUS_POLL_INTERVAL: Duration = Duration::from_millis(500);
const FULL_TON_IMAGE_INSPECT_TIMEOUT: Duration = Duration::from_secs(10);
const FULL_TON_IMAGE_PULL_TIMEOUT: Duration = Duration::from_secs(15 * 60);
const FULL_TON_COMPOSE_TIMEOUT: Duration = Duration::from_secs(15 * 60);
const PROJECT_ARTIFACT_POLL_INTERVAL: Duration = Duration::from_millis(750);
const PROJECT_ARTIFACT_DEBOUNCE: Duration = Duration::from_millis(500);
const PROJECT_ARTIFACT_PUBLISH_RETRY_INTERVAL: Duration = Duration::from_secs(2);

#[derive(Clone)]
pub struct LocalProcessEnvironmentRuntime {
    inner: Arc<LocalProcessRuntimeInner>,
}

struct LocalProcessRuntimeInner {
    acton_executable: PathBuf,
    workspace_root: PathBuf,
    next_id: AtomicU64,
    create_lock: Mutex<()>,
    environments: RwLock<Vec<Arc<LocalEnvironment>>>,
    artifact_synchronizer: ProjectArtifactSynchronizer,
    contract_registry: ContractRegistryStore,
    persistent_artifact_targets: Vec<String>,
    artifact_coordinator_started: AtomicBool,
    artifact_coordinator_wakeup: Notify,
    shutting_down: AtomicBool,
}

struct LocalEnvironment {
    details: RwLock<StudioEnvironment>,
    driver: EnvironmentDriver,
    child: Mutex<Option<Child>>,
    lifecycle: Mutex<()>,
    generation: AtomicU64,
    resume_on_startup: AtomicBool,
    deleted: AtomicBool,
}

enum EnvironmentDriver {
    ActonLocalnet {
        acton_executable: PathBuf,
        workspace_root: PathBuf,
        db_path: PathBuf,
        config: EnvironmentConfig,
        port: u16,
    },
    FullTonNetwork(FullTonNetworkDriver),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FullTonStartPhase {
    LocalImageCheck,
    ImagePull(FullTonImagePullKind),
    ComposeUp,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FullTonProcessOutcome {
    Succeeded,
    Failed,
    TimedOut,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FullTonTransition {
    StartImagePull,
    StartCompose,
    Running,
    Failed { cleanup_compose: bool },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FullTonImagePullKind {
    ActiveDockerConfiguration,
    IsolatedPublicImage,
}

struct ArtifactRevision {
    id: u64,
    fingerprint: Option<ProjectFingerprint>,
    artifacts: Vec<VerifiedSourceRegistration>,
}

#[derive(Default)]
struct ArtifactCoordinatorState {
    next_revision: u64,
    current: Option<ArtifactRevision>,
    published_revisions: HashMap<String, (u64, u64)>,
}

impl ArtifactCoordinatorState {
    fn is_current_fingerprint(&self, fingerprint: &ProjectFingerprint) -> bool {
        self.current
            .as_ref()
            .and_then(|revision| revision.fingerprint.as_ref())
            == Some(fingerprint)
    }

    fn commit_if_stable(
        &mut self,
        before_build: ProjectFingerprint,
        after_build: &ProjectFingerprint,
        artifacts: Vec<VerifiedSourceRegistration>,
    ) -> bool {
        if before_build != *after_build {
            return false;
        }
        if self
            .current
            .as_ref()
            .is_none_or(|revision| revision.artifacts != artifacts)
        {
            self.next_revision += 1;
            self.current = Some(ArtifactRevision {
                id: self.next_revision,
                fingerprint: Some(before_build),
                artifacts,
            });
        } else if let Some(revision) = self.current.as_mut() {
            revision.fingerprint = Some(before_build);
        }
        true
    }

    fn refresh_history(&mut self, artifacts: Vec<VerifiedSourceRegistration>) {
        if self
            .current
            .as_ref()
            .is_some_and(|revision| revision.artifacts == artifacts)
        {
            return;
        }
        self.next_revision += 1;
        let fingerprint = self
            .current
            .as_ref()
            .and_then(|revision| revision.fingerprint.clone());
        self.current = Some(ArtifactRevision {
            id: self.next_revision,
            fingerprint,
            artifacts,
        });
    }

    fn needs_publish(&self, environment_id: &str, generation: u64) -> bool {
        let Some(revision) = &self.current else {
            return false;
        };
        self.published_revisions
            .get(environment_id)
            .is_none_or(|published| *published < (generation, revision.id))
    }

    fn mark_published(&mut self, environment_id: String, generation: u64, revision: u64) {
        self.published_revisions
            .insert(environment_id, (generation, revision));
    }
}

impl LocalProcessEnvironmentRuntime {
    pub async fn open(
        acton_executable: impl Into<PathBuf>,
        workspace_root: impl Into<PathBuf>,
        contract_registry: ContractRegistryStore,
        mut persistent_artifact_targets: Vec<String>,
    ) -> Result<Self, EnvironmentRuntimeError> {
        let acton_executable = acton_executable.into();
        let workspace_root = workspace_root.into();
        persistent_artifact_targets.retain(|environment_id| !environment_id.is_empty());
        persistent_artifact_targets.sort();
        persistent_artifact_targets.dedup();
        let LoadedEnvironments { records, next_id } = load_environments(&workspace_root).await?;
        let inner = Arc::new(LocalProcessRuntimeInner {
            artifact_synchronizer: ProjectArtifactSynchronizer::new(
                acton_executable.clone(),
                workspace_root.clone(),
            ),
            contract_registry,
            persistent_artifact_targets,
            acton_executable,
            workspace_root,
            next_id: AtomicU64::new(next_id),
            create_lock: Mutex::new(()),
            environments: RwLock::new(Vec::with_capacity(records.len())),
            artifact_coordinator_started: AtomicBool::new(false),
            artifact_coordinator_wakeup: Notify::new(),
            shutting_down: AtomicBool::new(false),
        });

        for record in records {
            restore_environment(&inner, record).await?;
        }
        if !inner.persistent_artifact_targets.is_empty() {
            schedule_project_artifact_sync(&inner);
        }

        Ok(Self { inner })
    }
}

impl EnvironmentRuntime for LocalProcessEnvironmentRuntime {
    fn list(&self) -> EnvironmentRuntimeFuture<'_, Vec<StudioEnvironment>> {
        Box::pin(async move {
            let environments = self.inner.environments.read().await.clone();
            let mut result = Vec::with_capacity(environments.len());
            for environment in environments {
                result.push(environment.details.read().await.clone());
            }
            Ok(result)
        })
    }

    fn create(
        &self,
        request: CreateEnvironmentRequest,
    ) -> EnvironmentRuntimeFuture<'_, StudioEnvironment> {
        Box::pin(async move {
            let _create_guard = self.inner.create_lock.lock().await;
            let reserved_ports = reserved_environment_ports(&self.inner).await;
            let (name, config) = resolve_request(request, &reserved_ports)?;
            let id_number = self
                .inner
                .next_id
                .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |next_id| {
                    next_id.checked_add(1)
                })
                .map_err(|_| EnvironmentRuntimeError::Internal {
                    code: "environment_store_id_exhausted",
                    message: "Studio cannot allocate another environment id".to_owned(),
                })?;
            let id = format!("environment-{id_number}");
            let data_dir = environment_data_dir(&self.inner.workspace_root, &id);
            tokio::fs::create_dir_all(&data_dir)
                .await
                .map_err(|error| EnvironmentRuntimeError::Internal {
                    code: "environment_storage_failed",
                    message: format!(
                        "Failed to create environment storage at {}: {error}",
                        data_dir.display()
                    ),
                })?;

            let driver = match EnvironmentDriver::new(
                &self.inner.acton_executable,
                &self.inner.workspace_root,
                &data_dir,
                &id,
                &config,
            )
            .await
            {
                Ok(driver) => driver,
                Err(error) => {
                    let _ = tokio::fs::remove_dir_all(&data_dir).await;
                    return Err(error);
                }
            };
            let runtime_endpoints = runtime_endpoints(&config);
            let environment = Arc::new(LocalEnvironment {
                details: RwLock::new(StudioEnvironment::new(
                    id,
                    name,
                    EnvironmentStatus::Starting,
                    config,
                    runtime_endpoints,
                )),
                driver,
                child: Mutex::new(None),
                lifecycle: Mutex::new(()),
                generation: AtomicU64::new(1),
                resume_on_startup: AtomicBool::new(true),
                deleted: AtomicBool::new(false),
            });
            if let Err(error) =
                persist_environment_definition(&self.inner, &environment, true).await
            {
                let _ = tokio::fs::remove_dir_all(&data_dir).await;
                return Err(error);
            }
            let should_monitor = match environment.driver.spawn_start() {
                Ok(child) => {
                    *environment.child.lock().await = Some(child);
                    true
                }
                Err(error) => {
                    set_environment_status(
                        &environment,
                        EnvironmentStatus::Failed,
                        Some(error.to_string()),
                    )
                    .await;
                    false
                }
            };
            let result = environment.details.read().await.clone();
            self.inner
                .environments
                .write()
                .await
                .push(Arc::clone(&environment));
            if should_monitor {
                spawn_environment_monitor(Arc::clone(&self.inner), environment, 1);
            }
            Ok(result)
        })
    }

    fn get(&self, environment_id: &str) -> EnvironmentRuntimeFuture<'_, StudioEnvironment> {
        let environment_id = environment_id.to_owned();
        Box::pin(async move {
            let environment = find_environment(&self.inner, &environment_id).await?;
            Ok(environment.details.read().await.clone())
        })
    }

    fn update(
        &self,
        environment_id: &str,
        request: UpdateEnvironmentRequest,
    ) -> EnvironmentRuntimeFuture<'_, StudioEnvironment> {
        let environment_id = environment_id.to_owned();
        Box::pin(async move {
            let name = validate_environment_name(&request.name)?;
            let environment = find_environment(&self.inner, &environment_id).await?;
            let _lifecycle_guard = environment.lifecycle.lock().await;
            ensure_environment_not_deleted(&environment).await?;
            let details = environment.details.read().await.clone();
            persist_environment(
                &self.inner.workspace_root,
                &StoredEnvironment {
                    id: details.id,
                    name: name.clone(),
                    config: details.config,
                    resume_on_startup: environment.resume_on_startup.load(Ordering::Acquire),
                },
            )
            .await?;
            let mut details = environment.details.write().await;
            details.name = name;
            Ok(details.clone())
        })
    }

    fn delete(&self, environment_id: &str) -> EnvironmentRuntimeFuture<'_, ()> {
        let environment_id = environment_id.to_owned();
        Box::pin(async move {
            let environment = find_environment(&self.inner, &environment_id).await?;
            delete_environment_runtime(&self.inner, &environment).await?;
            Ok(())
        })
    }

    fn stop(&self, environment_id: &str) -> EnvironmentRuntimeFuture<'_, StudioEnvironment> {
        let environment_id = environment_id.to_owned();
        Box::pin(async move {
            let environment = find_environment(&self.inner, &environment_id).await?;
            stop_environment(&self.inner, &environment, true).await?;
            Ok(environment.details.read().await.clone())
        })
    }

    fn restart(&self, environment_id: &str) -> EnvironmentRuntimeFuture<'_, StudioEnvironment> {
        let environment_id = environment_id.to_owned();
        Box::pin(async move {
            let environment = find_environment(&self.inner, &environment_id).await?;
            restart_environment(&self.inner, &environment).await
        })
    }

    fn shutdown(&self) -> EnvironmentRuntimeFuture<'_, ()> {
        Box::pin(async move {
            self.inner.shutting_down.store(true, Ordering::Release);
            self.inner.artifact_coordinator_wakeup.notify_one();
            let environments = self.inner.environments.read().await.clone();
            let mut first_error = None;
            for environment in environments {
                if let Err(error) = stop_environment(&self.inner, &environment, false).await
                    && first_error.is_none()
                {
                    first_error = Some(error);
                }
            }
            if let Some(error) = first_error {
                return Err(error);
            }
            Ok(())
        })
    }
}

fn resolve_request(
    request: CreateEnvironmentRequest,
    reserved_ports: &[u16],
) -> Result<(String, EnvironmentConfig), EnvironmentRuntimeError> {
    let name = validate_environment_name(&request.name)?;
    let config = match request.config {
        CreateEnvironmentConfig::ActonLocalnet {
            port,
            mut fork_network,
            fork_block_number,
            accounts,
            rate_limit,
            response_delay_ms,
            block_interval_ms,
            no_mining,
            mine_empty_blocks,
        } => {
            validate_requested_port(port)?;
            fork_network = fork_network
                .map(|network| network.trim().to_owned())
                .filter(|network| !network.is_empty());
            if fork_block_number.is_some() && fork_network.is_none() {
                return Err(EnvironmentRuntimeError::InvalidRequest {
                    code: "fork_network_required",
                    message: "Fork network is required when a fork block is selected".to_owned(),
                });
            }
            if rate_limit == Some(0) || response_delay_ms == Some(0) || block_interval_ms == Some(0)
            {
                return Err(EnvironmentRuntimeError::InvalidRequest {
                    code: "environment_limit_invalid",
                    message:
                        "Rate limit, response delay and block interval must be greater than zero"
                            .to_owned(),
                });
            }
            if no_mining && mine_empty_blocks {
                return Err(EnvironmentRuntimeError::InvalidRequest {
                    code: "environment_mining_mode_invalid",
                    message: "Empty blocks cannot be mined while automatic mining is disabled"
                        .to_owned(),
                });
            }

            EnvironmentConfig::ActonLocalnet {
                port: select_port(FIRST_LOCALNET_PORT, port, reserved_ports)?,
                fork_network,
                fork_block_number,
                accounts: accounts
                    .into_iter()
                    .map(|account| account.trim().to_owned())
                    .filter(|account| !account.is_empty())
                    .collect(),
                rate_limit,
                response_delay_ms,
                block_interval_ms,
                no_mining,
                mine_empty_blocks,
            }
        }
        CreateEnvironmentConfig::FullTonNetwork {
            api_v2_port,
            api_v3_port,
            validators,
        } => {
            validate_requested_port(api_v2_port)?;
            validate_requested_port(api_v3_port)?;
            let api_v2_port = select_port(FIRST_FULL_TON_V2_PORT, api_v2_port, reserved_ports)?;
            if api_v3_port == Some(api_v2_port) {
                return Err(EnvironmentRuntimeError::InvalidRequest {
                    code: "environment_ports_must_differ",
                    message: "V2 and V3 API ports must be different".to_owned(),
                });
            }
            let mut excluded_ports = reserved_ports.to_vec();
            excluded_ports.push(api_v2_port);
            let api_v3_port = select_port(FIRST_FULL_TON_V3_PORT, api_v3_port, &excluded_ports)?;
            let validators = validators.unwrap_or(DEFAULT_FULL_TON_VALIDATORS);
            if !(1..=MAX_FULL_TON_VALIDATORS).contains(&validators) {
                return Err(EnvironmentRuntimeError::InvalidRequest {
                    code: "environment_validators_invalid",
                    message: format!(
                        "Validator count must be between 1 and {MAX_FULL_TON_VALIDATORS}"
                    ),
                });
            }
            EnvironmentConfig::FullTonNetwork {
                api_v2_port,
                api_v3_port,
                validators,
            }
        }
    };
    Ok((name, config))
}

fn validate_requested_port(port: Option<u16>) -> Result<(), EnvironmentRuntimeError> {
    if port == Some(0) {
        return Err(EnvironmentRuntimeError::InvalidRequest {
            code: "environment_port_invalid",
            message: "Environment ports must be greater than zero".to_owned(),
        });
    }
    Ok(())
}

fn validate_environment_name(name: &str) -> Result<String, EnvironmentRuntimeError> {
    let name = name.trim();
    if name.is_empty() {
        return Err(EnvironmentRuntimeError::InvalidRequest {
            code: "environment_name_required",
            message: "Environment name is required".to_owned(),
        });
    }
    if name.chars().count() > 80 {
        return Err(EnvironmentRuntimeError::InvalidRequest {
            code: "environment_name_too_long",
            message: "Environment name must contain at most 80 characters".to_owned(),
        });
    }
    Ok(name.to_owned())
}

fn select_port(
    first_port: u16,
    requested: Option<u16>,
    excluded: &[u16],
) -> Result<u16, EnvironmentRuntimeError> {
    if let Some(port) = requested {
        return if !excluded.contains(&port) && port_is_available(port) {
            Ok(port)
        } else {
            Err(EnvironmentRuntimeError::Conflict {
                code: "environment_port_unavailable",
                message: format!("Port {port} is already in use"),
            })
        };
    }

    (first_port..=u16::MAX)
        .find(|port| !excluded.contains(port) && port_is_available(*port))
        .ok_or_else(|| EnvironmentRuntimeError::Conflict {
            code: "environment_port_unavailable",
            message: "No local port is available for a new environment".to_owned(),
        })
}

fn port_is_available(port: u16) -> bool {
    StdTcpListener::bind((Ipv4Addr::LOCALHOST, port)).is_ok()
}

async fn reserved_environment_ports(runtime: &LocalProcessRuntimeInner) -> Vec<u16> {
    let environments = runtime.environments.read().await.clone();
    let mut ports = Vec::with_capacity(environments.len() * 2);
    for environment in environments {
        match &environment.details.read().await.config {
            EnvironmentConfig::ActonLocalnet { port, .. } => ports.push(*port),
            EnvironmentConfig::FullTonNetwork {
                api_v2_port,
                api_v3_port,
                ..
            } => {
                ports.push(*api_v2_port);
                ports.push(*api_v3_port);
            }
            EnvironmentConfig::RemoteTonNetwork { .. } => {}
        }
    }
    ports
}

fn runtime_endpoints(config: &EnvironmentConfig) -> EnvironmentEndpoints {
    match config {
        EnvironmentConfig::ActonLocalnet { port, .. } => {
            let root = format!("http://127.0.0.1:{port}");
            EnvironmentEndpoints {
                api_v2: Some(format!("{root}/api/v2")),
                api_v3: Some(format!("{root}/api/v3")),
                control: Some(root),
            }
        }
        EnvironmentConfig::FullTonNetwork {
            api_v2_port,
            api_v3_port,
            ..
        } => EnvironmentEndpoints {
            api_v2: Some(format!("http://127.0.0.1:{api_v2_port}/api/v2")),
            api_v3: Some(format!("http://127.0.0.1:{api_v3_port}/api/v3")),
            control: None,
        },
        EnvironmentConfig::RemoteTonNetwork { .. } => EnvironmentEndpoints::default(),
    }
}

fn environment_data_dir(workspace_root: &Path, environment_id: &str) -> PathBuf {
    workspace_root
        .join(".studio")
        .join("environments")
        .join(environment_id)
}

async fn restore_environment(
    runtime: &Arc<LocalProcessRuntimeInner>,
    record: StoredEnvironment,
) -> Result<(), EnvironmentRuntimeError> {
    let data_dir = environment_data_dir(&runtime.workspace_root, &record.id);
    let driver = EnvironmentDriver::new(
        &runtime.acton_executable,
        &runtime.workspace_root,
        &data_dir,
        &record.id,
        &record.config,
    )
    .await?;
    let runtime_endpoints = runtime_endpoints(&record.config);
    let (status, error, child) = if record.resume_on_startup {
        match driver
            .ensure_restartable()
            .and_then(|()| driver.spawn_start())
        {
            Ok(child) => (EnvironmentStatus::Starting, None, Some(child)),
            Err(error) => {
                tracing::warn!(
                    environment_id = %record.id,
                    %error,
                    "Failed to resume Studio environment"
                );
                (EnvironmentStatus::Failed, Some(error.to_string()), None)
            }
        }
    } else {
        (EnvironmentStatus::Stopped, None, None)
    };
    let environment = Arc::new(LocalEnvironment {
        details: RwLock::new(StudioEnvironment::new(
            record.id,
            record.name,
            status,
            record.config,
            runtime_endpoints,
        )),
        driver,
        child: Mutex::new(child),
        lifecycle: Mutex::new(()),
        generation: AtomicU64::new(1),
        resume_on_startup: AtomicBool::new(record.resume_on_startup),
        deleted: AtomicBool::new(false),
    });
    if let Some(error) = error {
        environment.details.write().await.error = Some(error);
    }
    runtime
        .environments
        .write()
        .await
        .push(Arc::clone(&environment));
    let should_monitor = environment.child.lock().await.is_some();
    if should_monitor {
        spawn_environment_monitor(Arc::clone(runtime), environment, 1);
    }
    Ok(())
}

async fn persist_environment_definition(
    runtime: &LocalProcessRuntimeInner,
    environment: &LocalEnvironment,
    resume_on_startup: bool,
) -> Result<(), EnvironmentRuntimeError> {
    let details = environment.details.read().await;
    persist_environment(
        &runtime.workspace_root,
        &StoredEnvironment {
            id: details.id.clone(),
            name: details.name.clone(),
            config: details.config.clone(),
            resume_on_startup,
        },
    )
    .await
}

impl EnvironmentDriver {
    async fn new(
        acton_executable: &Path,
        workspace_root: &Path,
        data_dir: &Path,
        environment_id: &str,
        config: &EnvironmentConfig,
    ) -> Result<Self, EnvironmentRuntimeError> {
        match config {
            EnvironmentConfig::ActonLocalnet { port, .. } => Ok(Self::ActonLocalnet {
                acton_executable: acton_executable.to_owned(),
                workspace_root: workspace_root.to_owned(),
                db_path: data_dir.join("localnet.sqlite"),
                config: config.clone(),
                port: *port,
            }),
            EnvironmentConfig::FullTonNetwork {
                api_v2_port,
                api_v3_port,
                validators,
            } => FullTonNetworkDriver::materialize(
                data_dir,
                environment_id,
                *api_v2_port,
                *api_v3_port,
                *validators,
            )
            .await
            .map(Self::FullTonNetwork),
            EnvironmentConfig::RemoteTonNetwork { .. } => {
                Err(EnvironmentRuntimeError::InvalidRequest {
                    code: "external_environment_not_managed",
                    message: "External TON networks are not managed by the local process runtime"
                        .to_owned(),
                })
            }
        }
    }

    fn spawn_start(&self) -> Result<Child, EnvironmentRuntimeError> {
        match self {
            Self::ActonLocalnet {
                acton_executable,
                workspace_root,
                db_path,
                config,
                ..
            } => spawn_localnet(acton_executable, workspace_root, db_path.clone(), config),
            Self::FullTonNetwork(driver) => driver.spawn_image_inspect(),
        }
    }

    fn ensure_restartable(&self) -> Result<(), EnvironmentRuntimeError> {
        match self {
            Self::ActonLocalnet { port, .. } => select_port(*port, Some(*port), &[]).map(|_| ()),
            Self::FullTonNetwork(_) => Ok(()),
        }
    }

    async fn stop(&self) -> Result<(), EnvironmentRuntimeError> {
        match self {
            Self::ActonLocalnet { .. } => Ok(()),
            Self::FullTonNetwork(driver) => driver.stop().await,
        }
    }

    async fn delete(&self) -> Result<(), EnvironmentRuntimeError> {
        match self {
            Self::ActonLocalnet { .. } => Ok(()),
            Self::FullTonNetwork(driver) => driver.delete().await,
        }
    }

    async fn monitor(
        &self,
        runtime: Arc<LocalProcessRuntimeInner>,
        environment: Arc<LocalEnvironment>,
        generation: u64,
    ) {
        match self {
            Self::ActonLocalnet { port, .. } => {
                monitor_localnet(runtime, environment, generation, *port).await;
            }
            Self::FullTonNetwork(driver) => {
                monitor_full_ton_network(driver, runtime, environment, generation).await;
            }
        }
    }
}

fn spawn_localnet(
    acton_executable: &Path,
    workspace_root: &Path,
    db_path: PathBuf,
    config: &EnvironmentConfig,
) -> Result<Child, EnvironmentRuntimeError> {
    let EnvironmentConfig::ActonLocalnet {
        port,
        fork_network,
        fork_block_number,
        accounts,
        rate_limit,
        response_delay_ms,
        block_interval_ms,
        no_mining,
        mine_empty_blocks,
    } = config
    else {
        unreachable!("localnet driver requires an Acton localnet configuration");
    };
    let mut command = Command::new(acton_executable);
    command
        .arg("--project-root")
        .arg(workspace_root)
        .arg("localnet")
        .arg("start")
        .arg("--port")
        .arg(port.to_string())
        .arg("--db-path")
        .arg(db_path)
        .current_dir(workspace_root)
        .stdin(Stdio::null())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .kill_on_drop(true);

    if let Some(fork_network) = fork_network {
        command.arg("--fork-net").arg(fork_network);
    }
    if let Some(fork_block_number) = fork_block_number {
        command
            .arg("--fork-block-number")
            .arg(fork_block_number.to_string());
    }
    if !accounts.is_empty() {
        command.arg("--accounts").arg(accounts.join(","));
    }
    if let Some(rate_limit) = rate_limit {
        command.arg("--rate-limit").arg(rate_limit.to_string());
    }
    if let Some(response_delay_ms) = response_delay_ms {
        command
            .arg("--response-delay-ms")
            .arg(response_delay_ms.to_string());
    }
    if let Some(block_interval_ms) = block_interval_ms {
        command
            .arg("--block-interval-ms")
            .arg(block_interval_ms.to_string());
    }
    if *no_mining {
        command.arg("--no-mining");
    }
    if *mine_empty_blocks {
        command.arg("--mine-empty-blocks");
    }

    command
        .spawn()
        .map_err(|error| EnvironmentRuntimeError::Internal {
            code: "environment_start_failed",
            message: format!(
                "Failed to start {} localnet: {error}",
                acton_executable.display()
            ),
        })
}

async fn find_environment(
    runtime: &LocalProcessRuntimeInner,
    environment_id: &str,
) -> Result<Arc<LocalEnvironment>, EnvironmentRuntimeError> {
    let environments = runtime.environments.read().await.clone();
    for environment in environments {
        if environment.details.read().await.id == environment_id {
            return Ok(environment);
        }
    }
    Err(EnvironmentRuntimeError::NotFound {
        environment_id: environment_id.to_owned(),
    })
}

async fn ensure_environment_not_deleted(
    environment: &LocalEnvironment,
) -> Result<(), EnvironmentRuntimeError> {
    if !environment.deleted.load(Ordering::Acquire) {
        return Ok(());
    }
    Err(EnvironmentRuntimeError::NotFound {
        environment_id: environment.details.read().await.id.clone(),
    })
}

fn spawn_environment_monitor(
    runtime: Arc<LocalProcessRuntimeInner>,
    environment: Arc<LocalEnvironment>,
    generation: u64,
) {
    tokio::spawn(async move {
        let monitored_environment = Arc::clone(&environment);
        environment
            .driver
            .monitor(runtime, monitored_environment, generation)
            .await;
    });
}

async fn monitor_localnet(
    runtime: Arc<LocalProcessRuntimeInner>,
    environment: Arc<LocalEnvironment>,
    generation: u64,
    port: u16,
) {
    let address = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), port);
    let ready_deadline = Instant::now() + LOCALNET_READY_TIMEOUT;

    loop {
        if !is_current_generation(&environment, generation) {
            return;
        }
        match child_exit_status(&environment).await {
            Ok(Some(status)) => {
                record_exit(&environment, generation, status).await;
                return;
            }
            Ok(None) => {}
            Err(error) => {
                fail_environment(
                    &environment,
                    generation,
                    format!("Failed to inspect localnet process: {error}"),
                )
                .await;
                return;
            }
        }

        if timeout(LOCALNET_READY_POLL_INTERVAL, TcpStream::connect(address))
            .await
            .is_ok_and(|result| result.is_ok())
        {
            set_environment_status_if_current(
                &environment,
                generation,
                EnvironmentStatus::Running,
                None,
            )
            .await;
            schedule_project_artifact_sync(&runtime);
            break;
        }

        if Instant::now() >= ready_deadline {
            fail_environment(
                &environment,
                generation,
                format!(
                    "Localnet did not become ready on port {port} within {} seconds",
                    LOCALNET_READY_TIMEOUT.as_secs()
                ),
            )
            .await;
            return;
        }
        sleep(LOCALNET_READY_POLL_INTERVAL).await;
    }

    loop {
        sleep(LOCALNET_STATUS_POLL_INTERVAL).await;
        if !is_current_generation(&environment, generation) {
            return;
        }
        match child_exit_status(&environment).await {
            Ok(Some(status)) => {
                record_exit(&environment, generation, status).await;
                return;
            }
            Ok(None) => {}
            Err(error) => {
                fail_environment(
                    &environment,
                    generation,
                    format!("Failed to inspect localnet process: {error}"),
                )
                .await;
                return;
            }
        }
    }
}

async fn monitor_full_ton_network(
    driver: &FullTonNetworkDriver,
    runtime: Arc<LocalProcessRuntimeInner>,
    environment: Arc<LocalEnvironment>,
    generation: u64,
) {
    let mut phase = FullTonStartPhase::LocalImageCheck;
    let mut deadline = Instant::now() + FULL_TON_IMAGE_INSPECT_TIMEOUT;

    loop {
        if !is_current_generation(&environment, generation) {
            return;
        }

        let (outcome, exit_status) = if Instant::now() >= deadline {
            terminate_child(&environment).await;
            (FullTonProcessOutcome::TimedOut, None)
        } else {
            match child_exit_status(&environment).await {
                Ok(Some(status)) if status.success() => {
                    (FullTonProcessOutcome::Succeeded, Some(status))
                }
                Ok(Some(status)) => (FullTonProcessOutcome::Failed, Some(status)),
                Ok(None) => {
                    sleep(LOCALNET_STATUS_POLL_INTERVAL).await;
                    continue;
                }
                Err(error) => {
                    fail_full_ton_network(
                        driver,
                        &environment,
                        generation,
                        matches!(phase, FullTonStartPhase::ComposeUp),
                        format!("Failed to inspect the Docker startup process: {error}"),
                    )
                    .await;
                    return;
                }
            }
        };

        match full_ton_transition(phase, outcome) {
            FullTonTransition::StartImagePull => {
                match start_full_ton_image_pull(driver, &environment, generation).await {
                    Ok(Some(kind)) => {
                        phase = FullTonStartPhase::ImagePull(kind);
                        deadline = Instant::now() + FULL_TON_IMAGE_PULL_TIMEOUT;
                    }
                    Ok(None) => return,
                    Err(error) => {
                        fail_environment(&environment, generation, error.to_string()).await;
                        return;
                    }
                }
            }
            FullTonTransition::StartCompose => {
                let started = match spawn_child_if_current(&environment, generation, || {
                    driver.spawn_compose_up()
                })
                .await
                {
                    Ok(started) => started,
                    Err(error) => {
                        fail_environment(&environment, generation, error.to_string()).await;
                        return;
                    }
                };
                if !started {
                    return;
                }
                phase = FullTonStartPhase::ComposeUp;
                deadline = Instant::now() + FULL_TON_COMPOSE_TIMEOUT;
            }
            FullTonTransition::Running => {
                set_environment_status_if_current(
                    &environment,
                    generation,
                    EnvironmentStatus::Running,
                    None,
                )
                .await;
                schedule_project_artifact_sync(&runtime);
                return;
            }
            FullTonTransition::Failed { cleanup_compose } => {
                let error = if let Some(status) = exit_status {
                    driver
                        .startup_failure_message(full_ton_operation(phase), status)
                        .await
                } else {
                    full_ton_timeout_message(phase)
                };
                fail_full_ton_network(driver, &environment, generation, cleanup_compose, error)
                    .await;
                return;
            }
        }
    }
}

const fn full_ton_transition(
    phase: FullTonStartPhase,
    outcome: FullTonProcessOutcome,
) -> FullTonTransition {
    match (phase, outcome) {
        (
            FullTonStartPhase::LocalImageCheck,
            FullTonProcessOutcome::Failed | FullTonProcessOutcome::TimedOut,
        ) => FullTonTransition::StartImagePull,
        (FullTonStartPhase::LocalImageCheck, FullTonProcessOutcome::Succeeded)
        | (FullTonStartPhase::ImagePull(_), FullTonProcessOutcome::Succeeded) => {
            FullTonTransition::StartCompose
        }
        (
            FullTonStartPhase::ImagePull(_),
            FullTonProcessOutcome::Failed | FullTonProcessOutcome::TimedOut,
        ) => FullTonTransition::Failed {
            cleanup_compose: false,
        },
        (FullTonStartPhase::ComposeUp, FullTonProcessOutcome::Succeeded) => {
            FullTonTransition::Running
        }
        (
            FullTonStartPhase::ComposeUp,
            FullTonProcessOutcome::Failed | FullTonProcessOutcome::TimedOut,
        ) => FullTonTransition::Failed {
            cleanup_compose: true,
        },
    }
}

const fn full_ton_operation(phase: FullTonStartPhase) -> &'static str {
    match phase {
        FullTonStartPhase::LocalImageCheck => "inspect the full TON network image with Docker",
        FullTonStartPhase::ImagePull(kind) => match kind {
            FullTonImagePullKind::IsolatedPublicImage => {
                "pull the public full TON network image using the isolated Docker configuration"
            }
            FullTonImagePullKind::ActiveDockerConfiguration => {
                "pull the full TON network image using the active Docker configuration"
            }
        },
        FullTonStartPhase::ComposeUp => "start the full TON network with Docker Compose",
    }
}

fn full_ton_timeout_message(phase: FullTonStartPhase) -> String {
    match phase {
        FullTonStartPhase::LocalImageCheck => {
            "Docker image inspection did not finish within 10 seconds".to_owned()
        }
        FullTonStartPhase::ImagePull(_) => {
            "Docker image pull did not finish within 15 minutes".to_owned()
        }
        FullTonStartPhase::ComposeUp => {
            "Full TON network startup did not finish within 15 minutes".to_owned()
        }
    }
}

async fn start_full_ton_image_pull(
    driver: &FullTonNetworkDriver,
    environment: &LocalEnvironment,
    generation: u64,
) -> Result<Option<FullTonImagePullKind>, EnvironmentRuntimeError> {
    if !is_current_generation(environment, generation) {
        return Ok(None);
    }

    let (kind, isolated_target) = match driver.isolated_pull_target().await {
        Ok(Some(target)) => (FullTonImagePullKind::IsolatedPublicImage, Some(target)),
        Ok(None) => (FullTonImagePullKind::ActiveDockerConfiguration, None),
        Err(error) => {
            tracing::warn!(
                %error,
                "Isolated Docker image pull is unavailable; using the active Docker configuration"
            );
            (FullTonImagePullKind::ActiveDockerConfiguration, None)
        }
    };

    let started = spawn_child_if_current(environment, generation, move || {
        if let Some(target) = isolated_target {
            driver.spawn_isolated_pull(&target)
        } else {
            driver.spawn_normal_pull()
        }
    })
    .await?;
    if started { Ok(Some(kind)) } else { Ok(None) }
}

async fn fail_full_ton_network(
    driver: &FullTonNetworkDriver,
    environment: &LocalEnvironment,
    generation: u64,
    cleanup_compose: bool,
    error: String,
) {
    let _lifecycle_guard = environment.lifecycle.lock().await;
    if !is_current_generation(environment, generation) {
        return;
    }
    terminate_child(environment).await;
    let mut error = error;
    if cleanup_compose && let Err(cleanup_error) = driver.stop().await {
        tracing::warn!(
            %cleanup_error,
            "Failed to stop partially started full TON network containers"
        );
        error = format!(
            "{error}\nCleanup failed; some full TON network containers may still be running: {cleanup_error}"
        );
    }
    set_environment_status_if_current(
        environment,
        generation,
        EnvironmentStatus::Failed,
        Some(error),
    )
    .await;
}

async fn spawn_child_if_current(
    environment: &LocalEnvironment,
    generation: u64,
    spawn: impl FnOnce() -> Result<Child, EnvironmentRuntimeError>,
) -> Result<bool, EnvironmentRuntimeError> {
    let _lifecycle_guard = environment.lifecycle.lock().await;
    if !is_current_generation(environment, generation) {
        return Ok(false);
    }
    let child = spawn()?;
    *environment.child.lock().await = Some(child);
    Ok(true)
}

fn schedule_project_artifact_sync(runtime: &Arc<LocalProcessRuntimeInner>) {
    if runtime.shutting_down.load(Ordering::Acquire) {
        return;
    }

    if runtime
        .artifact_coordinator_started
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_ok()
    {
        let coordinator_runtime = Arc::clone(runtime);
        tokio::spawn(async move {
            run_project_artifact_coordinator(&coordinator_runtime).await;
            coordinator_runtime
                .artifact_coordinator_started
                .store(false, Ordering::Release);
        });
    }
    runtime.artifact_coordinator_wakeup.notify_one();
}

async fn run_project_artifact_coordinator(runtime: &Arc<LocalProcessRuntimeInner>) {
    let mut state = ArtifactCoordinatorState::default();
    let mut observed_fingerprint = None;
    let mut changed_at = None;
    let mut failed_build_fingerprint = None;
    let mut next_publish_retry = Instant::now();

    loop {
        if runtime.shutting_down.load(Ordering::Acquire) {
            return;
        }
        if !has_artifact_publication_target(runtime).await {
            runtime.artifact_coordinator_wakeup.notified().await;
            failed_build_fingerprint = None;
            next_publish_retry = Instant::now();
            continue;
        }

        let fingerprint = match runtime.artifact_synchronizer.fingerprint().await {
            Ok(fingerprint) => fingerprint,
            Err(error) => {
                tracing::warn!(%error, "Failed to inspect Acton project artifacts");
                wait_for_project_artifact_event(runtime, &mut failed_build_fingerprint).await;
                continue;
            }
        };
        match runtime.artifact_synchronizer.load_history().await {
            Ok(artifacts) => state.refresh_history(artifacts),
            Err(error) => {
                tracing::warn!(%error, "Failed to restore Acton source artifact history");
                wait_for_project_artifact_event(runtime, &mut failed_build_fingerprint).await;
                continue;
            }
        }
        if observed_fingerprint.as_ref() != Some(&fingerprint) {
            observed_fingerprint = Some(fingerprint.clone());
            changed_at = Some(Instant::now());
        }

        let fingerprint_is_stable =
            changed_at.is_some_and(|changed_at| changed_at.elapsed() >= PROJECT_ARTIFACT_DEBOUNCE);
        if fingerprint_is_stable
            && !state.is_current_fingerprint(&fingerprint)
            && failed_build_fingerprint.as_ref() != Some(&fingerprint)
            && has_artifact_publication_target(runtime).await
        {
            match runtime.artifact_synchronizer.build_and_store().await {
                Ok(_) => match runtime.artifact_synchronizer.fingerprint().await {
                    Ok(after_build) => {
                        let history = match runtime.artifact_synchronizer.load_history().await {
                            Ok(history) => history,
                            Err(error) => {
                                tracing::warn!(
                                    %error,
                                    "Failed to restore Acton source artifact history after build"
                                );
                                wait_for_project_artifact_event(
                                    runtime,
                                    &mut failed_build_fingerprint,
                                )
                                .await;
                                continue;
                            }
                        };
                        if state.commit_if_stable(fingerprint.clone(), &after_build, history) {
                            failed_build_fingerprint = None;
                            next_publish_retry = Instant::now();
                        } else {
                            observed_fingerprint = Some(after_build);
                            changed_at = Some(Instant::now());
                            continue;
                        }
                    }
                    Err(error) => {
                        tracing::warn!(
                            %error,
                            "Failed to verify Acton project inputs after building artifacts"
                        );
                        observed_fingerprint = None;
                        changed_at = None;
                        wait_for_project_artifact_event(runtime, &mut failed_build_fingerprint)
                            .await;
                        continue;
                    }
                },
                Err(error) => {
                    tracing::warn!(%error, "Failed to build Acton project artifacts");
                    failed_build_fingerprint = Some(fingerprint.clone());
                }
            }
        }

        if state.current.is_some() && Instant::now() >= next_publish_retry {
            let had_failures = publish_current_artifact_revision(runtime, &mut state).await;
            next_publish_retry = if had_failures {
                Instant::now() + PROJECT_ARTIFACT_PUBLISH_RETRY_INTERVAL
            } else {
                Instant::now()
            };
        }

        wait_for_project_artifact_event(runtime, &mut failed_build_fingerprint).await;
    }
}

async fn wait_for_project_artifact_event(
    runtime: &Arc<LocalProcessRuntimeInner>,
    failed_build_fingerprint: &mut Option<ProjectFingerprint>,
) {
    tokio::select! {
        () = runtime.artifact_coordinator_wakeup.notified() => {
            *failed_build_fingerprint = None;
        }
        () = sleep(PROJECT_ARTIFACT_POLL_INTERVAL) => {}
    }
}

async fn publish_current_artifact_revision(
    runtime: &Arc<LocalProcessRuntimeInner>,
    state: &mut ArtifactCoordinatorState,
) -> bool {
    let Some(revision_id) = state.current.as_ref().map(|revision| revision.id) else {
        return false;
    };
    let mut had_failures = false;
    for (environment_id, generation) in artifact_publication_targets(runtime).await {
        if !state.needs_publish(&environment_id, generation) {
            continue;
        }
        let revision = state.current.as_ref().expect("revision exists");
        let result = runtime
            .contract_registry
            .register_verified_sources(&environment_id, &revision.artifacts)
            .await;
        match result {
            Ok(()) => {
                state.mark_published(environment_id.clone(), generation, revision_id);
                tracing::debug!(
                    environment_id,
                    artifact_count = state
                        .current
                        .as_ref()
                        .map_or(0, |revision| revision.artifacts.len()),
                    revision = revision_id,
                    "Synchronized Acton project artifacts"
                );
            }
            Err(error) => {
                had_failures = true;
                tracing::warn!(
                    environment_id,
                    %error,
                    "Failed to register Acton project artifacts"
                );
            }
        }
    }
    had_failures
}

async fn artifact_publication_targets(runtime: &LocalProcessRuntimeInner) -> Vec<(String, u64)> {
    let mut targets = runtime
        .persistent_artifact_targets
        .iter()
        .cloned()
        .map(|environment_id| (environment_id, 0))
        .collect::<HashMap<_, _>>();
    let environments = runtime.environments.read().await.clone();
    for environment in environments {
        let environment_id = {
            let details = environment.details.read().await;
            (details.status == EnvironmentStatus::Running).then(|| details.id.clone())
        };
        if let Some(environment_id) = environment_id {
            let generation = environment.generation.load(Ordering::Acquire);
            targets.insert(environment_id, generation);
        }
    }
    targets.into_iter().collect()
}

async fn has_artifact_publication_target(runtime: &LocalProcessRuntimeInner) -> bool {
    !artifact_publication_targets(runtime).await.is_empty()
}

async fn child_exit_status(environment: &LocalEnvironment) -> std::io::Result<Option<ExitStatus>> {
    let mut child = environment.child.lock().await;
    let Some(process) = child.as_mut() else {
        return Ok(None);
    };
    let status = process.try_wait()?;
    if status.is_some() {
        child.take();
    }
    drop(child);
    Ok(status)
}

async fn record_exit(environment: &LocalEnvironment, generation: u64, status: ExitStatus) {
    set_environment_status_if_current(
        environment,
        generation,
        EnvironmentStatus::Failed,
        Some(format!("Localnet exited with {status}")),
    )
    .await;
}

async fn stop_environment(
    runtime: &LocalProcessRuntimeInner,
    environment: &LocalEnvironment,
    persist_intent: bool,
) -> Result<(), EnvironmentRuntimeError> {
    let _lifecycle_guard = environment.lifecycle.lock().await;
    ensure_environment_not_deleted(environment).await?;
    let current_status = environment.details.read().await.status;
    if current_status == EnvironmentStatus::Stopped {
        if persist_intent {
            persist_environment_definition(runtime, environment, false).await?;
            environment
                .resume_on_startup
                .store(false, Ordering::Release);
        }
        return Ok(());
    }

    environment.generation.fetch_add(1, Ordering::AcqRel);
    set_environment_status(environment, EnvironmentStatus::Stopping, None).await;
    terminate_child(environment).await;
    if let Err(error) = environment.driver.stop().await {
        set_environment_status(
            environment,
            EnvironmentStatus::Failed,
            Some(error.to_string()),
        )
        .await;
        return Err(error);
    }
    if persist_intent {
        persist_environment_definition(runtime, environment, false).await?;
        environment
            .resume_on_startup
            .store(false, Ordering::Release);
    }
    set_environment_status(environment, EnvironmentStatus::Stopped, None).await;
    Ok(())
}

async fn delete_environment_runtime(
    runtime: &LocalProcessRuntimeInner,
    environment: &LocalEnvironment,
) -> Result<(), EnvironmentRuntimeError> {
    let _lifecycle_guard = environment.lifecycle.lock().await;
    ensure_environment_not_deleted(environment).await?;
    environment.generation.fetch_add(1, Ordering::AcqRel);
    set_environment_status(environment, EnvironmentStatus::Stopping, None).await;
    terminate_child(environment).await;
    if let Err(error) = environment.driver.delete().await {
        set_environment_status(
            environment,
            EnvironmentStatus::Failed,
            Some(error.to_string()),
        )
        .await;
        return Err(error);
    }
    set_environment_status(environment, EnvironmentStatus::Stopped, None).await;
    let environment_id = environment.details.read().await.id.clone();
    let data_dir = environment_data_dir(&runtime.workspace_root, &environment_id);
    if let Err(error) = tokio::fs::remove_dir_all(&data_dir).await
        && error.kind() != std::io::ErrorKind::NotFound
    {
        return Err(EnvironmentRuntimeError::Internal {
            code: "environment_storage_delete_failed",
            message: format!(
                "Failed to delete environment storage at {}: {error}",
                data_dir.display()
            ),
        });
    }
    environment.deleted.store(true, Ordering::Release);
    runtime
        .environments
        .write()
        .await
        .retain(|candidate| !std::ptr::eq(candidate.as_ref(), environment));
    Ok(())
}

async fn restart_environment(
    runtime: &Arc<LocalProcessRuntimeInner>,
    environment: &Arc<LocalEnvironment>,
) -> Result<StudioEnvironment, EnvironmentRuntimeError> {
    let _lifecycle_guard = environment.lifecycle.lock().await;
    ensure_environment_not_deleted(environment).await?;
    let details = environment.details.read().await.clone();
    if !matches!(
        details.status,
        EnvironmentStatus::Stopped | EnvironmentStatus::Failed
    ) {
        return Err(EnvironmentRuntimeError::Conflict {
            code: "environment_not_restartable",
            message: format!(
                "Environment {} must be stopped before it can be restarted",
                details.name
            ),
        });
    }

    persist_environment_definition(runtime, environment, true).await?;
    environment.resume_on_startup.store(true, Ordering::Release);
    if let Err(error) = environment.driver.ensure_restartable() {
        set_environment_status(
            environment,
            EnvironmentStatus::Failed,
            Some(error.to_string()),
        )
        .await;
        return Err(error);
    }
    environment.generation.fetch_add(1, Ordering::AcqRel);
    terminate_child(environment).await;
    let child = match environment.driver.spawn_start() {
        Ok(child) => child,
        Err(error) => {
            set_environment_status(
                environment,
                EnvironmentStatus::Failed,
                Some(error.to_string()),
            )
            .await;
            return Err(error);
        }
    };
    *environment.child.lock().await = Some(child);
    let generation = environment.generation.load(Ordering::Acquire);
    set_environment_status(environment, EnvironmentStatus::Starting, None).await;
    spawn_environment_monitor(Arc::clone(runtime), Arc::clone(environment), generation);
    Ok(environment.details.read().await.clone())
}

fn is_current_generation(environment: &LocalEnvironment, generation: u64) -> bool {
    environment.generation.load(Ordering::Acquire) == generation
}

async fn set_environment_status_if_current(
    environment: &LocalEnvironment,
    generation: u64,
    status: EnvironmentStatus,
    error: Option<String>,
) {
    let mut details = environment.details.write().await;
    if environment.generation.load(Ordering::Acquire) == generation {
        details.status = status;
        details.error = error;
    }
}

async fn fail_environment(environment: &LocalEnvironment, generation: u64, error: String) {
    let _lifecycle_guard = environment.lifecycle.lock().await;
    if !is_current_generation(environment, generation) {
        return;
    }
    terminate_child(environment).await;
    set_environment_status_if_current(
        environment,
        generation,
        EnvironmentStatus::Failed,
        Some(error),
    )
    .await;
}

async fn terminate_child(environment: &LocalEnvironment) {
    let process = environment.child.lock().await.take();
    let Some(mut process) = process else {
        return;
    };
    let _ = process.start_kill();
    let _ = process.wait().await;
}

async fn set_environment_status(
    environment: &LocalEnvironment,
    status: EnvironmentStatus,
    error: Option<String>,
) {
    let mut details = environment.details.write().await;
    details.status = status;
    details.error = error;
}

#[cfg(test)]
mod full_ton_start_tests {
    use std::path::PathBuf;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};

    use expect_test::expect;
    use tokio::sync::{Mutex, RwLock};

    use super::{
        EnvironmentConfig, EnvironmentDriver, EnvironmentEndpoints, EnvironmentRuntimeError,
        EnvironmentStatus, FullTonImagePullKind, FullTonProcessOutcome, FullTonStartPhase,
        LocalEnvironment, StudioEnvironment, full_ton_transition, spawn_child_if_current,
    };

    #[test]
    fn startup_transition_table_cleans_up_only_after_compose_started() {
        let image_pull = FullTonStartPhase::ImagePull(FullTonImagePullKind::IsolatedPublicImage);
        let cases = [
            (
                FullTonStartPhase::LocalImageCheck,
                FullTonProcessOutcome::Succeeded,
            ),
            (
                FullTonStartPhase::LocalImageCheck,
                FullTonProcessOutcome::Failed,
            ),
            (
                FullTonStartPhase::LocalImageCheck,
                FullTonProcessOutcome::TimedOut,
            ),
            (image_pull, FullTonProcessOutcome::Succeeded),
            (image_pull, FullTonProcessOutcome::Failed),
            (image_pull, FullTonProcessOutcome::TimedOut),
            (
                FullTonStartPhase::ComposeUp,
                FullTonProcessOutcome::Succeeded,
            ),
            (FullTonStartPhase::ComposeUp, FullTonProcessOutcome::Failed),
            (
                FullTonStartPhase::ComposeUp,
                FullTonProcessOutcome::TimedOut,
            ),
        ];
        let actual = cases
            .into_iter()
            .map(|(phase, outcome)| {
                format!(
                    "{phase:?} + {outcome:?} => {:?}",
                    full_ton_transition(phase, outcome)
                )
            })
            .collect::<Vec<_>>()
            .join("\n");

        expect![[r"LocalImageCheck + Succeeded => StartCompose
LocalImageCheck + Failed => StartImagePull
LocalImageCheck + TimedOut => StartImagePull
ImagePull(IsolatedPublicImage) + Succeeded => StartCompose
ImagePull(IsolatedPublicImage) + Failed => Failed { cleanup_compose: false }
ImagePull(IsolatedPublicImage) + TimedOut => Failed { cleanup_compose: false }
ComposeUp + Succeeded => Running
ComposeUp + Failed => Failed { cleanup_compose: true }
ComposeUp + TimedOut => Failed { cleanup_compose: true }"]]
        .assert_eq(&actual);
    }

    #[tokio::test]
    async fn cancelled_generation_cannot_spawn_the_next_startup_phase() {
        let environment = test_environment();
        let next_phase_spawned = Arc::new(AtomicBool::new(false));
        let lifecycle_guard = environment.lifecycle.lock().await;
        let task_environment = Arc::clone(&environment);
        let task_spawned = Arc::clone(&next_phase_spawned);
        let task = tokio::spawn(async move {
            spawn_child_if_current(&task_environment, 1, move || {
                task_spawned.store(true, Ordering::Release);
                Err(EnvironmentRuntimeError::Internal {
                    code: "unexpected_spawn",
                    message: "cancelled phase was spawned".to_owned(),
                })
            })
            .await
        });

        environment.generation.fetch_add(1, Ordering::AcqRel);
        drop(lifecycle_guard);

        let started = task
            .await
            .expect("startup task")
            .expect("cancelled phase must not be spawned");
        let actual = format!(
            "started: {started}\nspawn called: {}\nchild installed: {}",
            next_phase_spawned.load(Ordering::Acquire),
            environment.child.lock().await.is_some(),
        );
        expect![[r"started: false
spawn called: false
child installed: false"]]
        .assert_eq(&actual);
    }

    fn test_environment() -> Arc<LocalEnvironment> {
        let config = EnvironmentConfig::ActonLocalnet {
            port: 5411,
            fork_network: None,
            fork_block_number: None,
            accounts: Vec::new(),
            rate_limit: None,
            response_delay_ms: None,
            block_interval_ms: None,
            no_mining: false,
            mine_empty_blocks: false,
        };
        Arc::new(LocalEnvironment {
            details: RwLock::new(StudioEnvironment::new(
                "environment-1",
                "Test environment",
                EnvironmentStatus::Starting,
                config.clone(),
                EnvironmentEndpoints::default(),
            )),
            driver: EnvironmentDriver::ActonLocalnet {
                acton_executable: PathBuf::from("acton"),
                workspace_root: PathBuf::from("."),
                db_path: PathBuf::from("localnet.sqlite"),
                config,
                port: 5411,
            },
            child: Mutex::new(None),
            lifecycle: Mutex::new(()),
            generation: 1.into(),
            resume_on_startup: AtomicBool::new(true),
            deleted: AtomicBool::new(false),
        })
    }
}

#[cfg(test)]
mod external_environment_tests {
    use std::path::Path;

    use expect_test::expect;
    use tempfile::tempdir;

    use crate::environment::PublicTonNetwork;

    use super::{EnvironmentConfig, EnvironmentDriver, EnvironmentRuntimeError};

    #[tokio::test]
    async fn external_environment_is_rejected_without_materializing_a_data_directory() {
        let workspace = tempdir().expect("temporary workspace");
        let data_dir = workspace.path().join("testnet");
        let result = EnvironmentDriver::new(
            Path::new("acton"),
            workspace.path(),
            &data_dir,
            "testnet",
            &EnvironmentConfig::RemoteTonNetwork {
                network: PublicTonNetwork::Testnet,
            },
        )
        .await;
        let error = match result {
            Ok(_) => "unexpected success".to_owned(),
            Err(EnvironmentRuntimeError::InvalidRequest { code, message }) => {
                format!("{code} ({message})")
            }
            Err(error) => format!("unexpected error ({error})"),
        };
        let actual = format!(
            "result: {error}\ndata directory exists: {}",
            data_dir.exists()
        );

        expect![[r"result: external_environment_not_managed (External TON networks are not managed by the local process runtime)
data directory exists: false"]]
        .assert_eq(&actual);
    }
}

#[cfg(test)]
mod artifact_coordinator_tests {
    use std::fs;

    use serde_json::json;
    use tempfile::tempdir;

    use super::{
        ArtifactCoordinatorState, ProjectArtifactSynchronizer, VerifiedSourceRegistration,
    };

    #[tokio::test]
    async fn unstable_builds_are_discarded_and_publish_revisions_never_move_backwards() {
        let temp = tempdir().expect("temp directory");
        fs::write(temp.path().join("Acton.toml"), "[contracts]\n").expect("manifest");
        let contract_path = temp.path().join("counter.tolk");
        fs::write(&contract_path, "fun main() {}\n").expect("initial source");
        let synchronizer = ProjectArtifactSynchronizer::new("acton", temp.path());
        let before_build = synchronizer.fingerprint().await.expect("before build");
        fs::write(&contract_path, "fun noop() {}\n").expect("changed source");
        let after_build = synchronizer.fingerprint().await.expect("after build");

        let mut state = ArtifactCoordinatorState::default();
        assert!(!state.commit_if_stable(
            before_build,
            &after_build,
            vec![VerifiedSourceRegistration {
                code_hash: "stale".to_owned(),
                source: json!({"revision": "stale"}),
            }],
        ));
        assert!(state.current.is_none());

        assert!(state.commit_if_stable(
            after_build.clone(),
            &after_build,
            vec![VerifiedSourceRegistration {
                code_hash: "revision-1".to_owned(),
                source: json!({"revision": 1}),
            }],
        ));
        let revision_1 = state.current.as_ref().expect("first revision").id;
        state.mark_published("environment-1".to_owned(), 1, revision_1);
        assert!(!state.needs_publish("environment-1", 1));
        assert!(state.needs_publish("environment-1", 2));

        fs::write(&contract_path, "fun next() {}\n").expect("next source");
        let next_fingerprint = synchronizer.fingerprint().await.expect("next fingerprint");
        assert!(state.commit_if_stable(
            next_fingerprint.clone(),
            &next_fingerprint,
            vec![VerifiedSourceRegistration {
                code_hash: "revision-2".to_owned(),
                source: json!({"revision": 2}),
            }],
        ));
        let revision_2 = state.current.as_ref().expect("second revision").id;
        state.mark_published("environment-1".to_owned(), 1, revision_1);
        assert!(state.needs_publish("environment-1", 1));
        state.mark_published("environment-1".to_owned(), 1, revision_2);
        assert!(!state.needs_publish("environment-1", 1));
    }

    #[test]
    fn restored_history_is_published_before_a_workspace_rebuild() {
        let mut state = ArtifactCoordinatorState::default();
        state.refresh_history(vec![
            VerifiedSourceRegistration {
                code_hash: "old-code".to_owned(),
                source: json!({"bundle": "old"}),
            },
            VerifiedSourceRegistration {
                code_hash: "new-code".to_owned(),
                source: json!({"bundle": "new"}),
            },
        ]);

        assert!(state.current.is_some());
        assert!(state.needs_publish("environment-1", 1));
        assert!(
            state
                .current
                .as_ref()
                .expect("history revision")
                .fingerprint
                .is_none()
        );
    }
}
