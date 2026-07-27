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

use crate::environment::{
    CreateEnvironmentRequest, EnvironmentConfig, EnvironmentRuntime, EnvironmentRuntimeError,
    EnvironmentRuntimeFuture, EnvironmentStatus, StudioEnvironment, UpdateEnvironmentRequest,
};
use crate::local_artifacts::{ProjectArtifactSynchronizer, ProjectFingerprint, SourceRegistration};

const FIRST_LOCALNET_PORT: u16 = 5411;
const LOCALNET_READY_TIMEOUT: Duration = Duration::from_secs(15);
const LOCALNET_READY_POLL_INTERVAL: Duration = Duration::from_millis(100);
const LOCALNET_STATUS_POLL_INTERVAL: Duration = Duration::from_millis(500);
const PROJECT_ARTIFACT_POLL_INTERVAL: Duration = Duration::from_millis(750);
const PROJECT_ARTIFACT_DEBOUNCE: Duration = Duration::from_millis(500);
const PROJECT_ARTIFACT_REGISTER_RETRY_INTERVAL: Duration = Duration::from_secs(2);

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
    artifact_coordinator_started: AtomicBool,
    artifact_coordinator_wakeup: Notify,
    shutting_down: AtomicBool,
}

struct LocalEnvironment {
    details: RwLock<StudioEnvironment>,
    child: Mutex<Option<Child>>,
    lifecycle: Mutex<()>,
    generation: AtomicU64,
}

struct ArtifactRevision {
    id: u64,
    fingerprint: Option<ProjectFingerprint>,
    artifacts: Vec<SourceRegistration>,
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
        artifacts: Vec<SourceRegistration>,
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

    fn refresh_history(&mut self, artifacts: Vec<SourceRegistration>) {
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
    #[must_use]
    pub fn new(acton_executable: impl Into<PathBuf>, workspace_root: impl Into<PathBuf>) -> Self {
        let acton_executable = acton_executable.into();
        let workspace_root = workspace_root.into();
        Self {
            inner: Arc::new(LocalProcessRuntimeInner {
                artifact_synchronizer: ProjectArtifactSynchronizer::new(
                    acton_executable.clone(),
                    workspace_root.clone(),
                ),
                acton_executable,
                workspace_root,
                next_id: AtomicU64::new(1),
                create_lock: Mutex::new(()),
                environments: RwLock::new(Vec::new()),
                artifact_coordinator_started: AtomicBool::new(false),
                artifact_coordinator_wakeup: Notify::new(),
                shutting_down: AtomicBool::new(false),
            }),
        }
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
            let request = validate_request(request)?;
            let _create_guard = self.inner.create_lock.lock().await;
            let port = select_port(request.port)?;
            let id = format!(
                "environment-{}",
                self.inner.next_id.fetch_add(1, Ordering::Relaxed)
            );
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

            let config = EnvironmentConfig {
                port,
                fork_network: request.fork_network,
                fork_block_number: request.fork_block_number,
                accounts: request.accounts,
                rate_limit: request.rate_limit,
                response_delay_ms: request.response_delay_ms,
                block_interval_ms: request.block_interval_ms,
                no_mining: request.no_mining,
                mine_empty_blocks: request.mine_empty_blocks,
            };
            let rpc_url = format!("http://127.0.0.1:{port}");
            let child = spawn_localnet(
                &self.inner.acton_executable,
                &self.inner.workspace_root,
                data_dir.join("localnet.sqlite"),
                &config,
            )?;
            let environment = Arc::new(LocalEnvironment {
                details: RwLock::new(StudioEnvironment {
                    id,
                    name: request.name,
                    status: EnvironmentStatus::Starting,
                    rpc_url,
                    config,
                    error: None,
                }),
                child: Mutex::new(Some(child)),
                lifecycle: Mutex::new(()),
                generation: AtomicU64::new(1),
            });
            let result = environment.details.read().await.clone();
            self.inner
                .environments
                .write()
                .await
                .push(Arc::clone(&environment));
            tokio::spawn(monitor_localnet(Arc::clone(&self.inner), environment, 1));
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
            let mut details = environment.details.write().await;
            details.name = name;
            Ok(details.clone())
        })
    }

    fn delete(&self, environment_id: &str) -> EnvironmentRuntimeFuture<'_, ()> {
        let environment_id = environment_id.to_owned();
        Box::pin(async move {
            let environment = find_environment(&self.inner, &environment_id).await?;
            stop_environment(&environment).await;

            let data_dir = environment_data_dir(&self.inner.workspace_root, &environment_id);
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

            self.inner
                .environments
                .write()
                .await
                .retain(|candidate| !Arc::ptr_eq(candidate, &environment));
            Ok(())
        })
    }

    fn stop(&self, environment_id: &str) -> EnvironmentRuntimeFuture<'_, StudioEnvironment> {
        let environment_id = environment_id.to_owned();
        Box::pin(async move {
            let environment = find_environment(&self.inner, &environment_id).await?;
            stop_environment(&environment).await;
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
            for environment in environments {
                stop_environment(&environment).await;
            }
            Ok(())
        })
    }
}

fn validate_request(
    mut request: CreateEnvironmentRequest,
) -> Result<CreateEnvironmentRequest, EnvironmentRuntimeError> {
    request.name = validate_environment_name(&request.name)?;
    if request.port == Some(0) {
        return Err(EnvironmentRuntimeError::InvalidRequest {
            code: "environment_port_invalid",
            message: "Environment port must be greater than zero".to_owned(),
        });
    }

    request.fork_network = request
        .fork_network
        .map(|network| network.trim().to_owned())
        .filter(|network| !network.is_empty());
    if request.fork_block_number.is_some() && request.fork_network.is_none() {
        return Err(EnvironmentRuntimeError::InvalidRequest {
            code: "fork_network_required",
            message: "Fork network is required when a fork block is selected".to_owned(),
        });
    }
    if request.rate_limit == Some(0)
        || request.response_delay_ms == Some(0)
        || request.block_interval_ms == Some(0)
    {
        return Err(EnvironmentRuntimeError::InvalidRequest {
            code: "environment_limit_invalid",
            message: "Rate limit, response delay and block interval must be greater than zero"
                .to_owned(),
        });
    }
    if request.no_mining && request.mine_empty_blocks {
        return Err(EnvironmentRuntimeError::InvalidRequest {
            code: "environment_mining_mode_invalid",
            message: "Empty blocks cannot be mined while automatic mining is disabled".to_owned(),
        });
    }

    request.accounts = request
        .accounts
        .into_iter()
        .map(|account| account.trim().to_owned())
        .filter(|account| !account.is_empty())
        .collect();
    Ok(request)
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

fn select_port(requested: Option<u16>) -> Result<u16, EnvironmentRuntimeError> {
    if let Some(port) = requested {
        return if port_is_available(port) {
            Ok(port)
        } else {
            Err(EnvironmentRuntimeError::Conflict {
                code: "environment_port_unavailable",
                message: format!("Port {port} is already in use"),
            })
        };
    }

    (FIRST_LOCALNET_PORT..=u16::MAX)
        .find(|port| port_is_available(*port))
        .ok_or_else(|| EnvironmentRuntimeError::Conflict {
            code: "environment_port_unavailable",
            message: "No local port is available for a new environment".to_owned(),
        })
}

fn port_is_available(port: u16) -> bool {
    StdTcpListener::bind((Ipv4Addr::LOCALHOST, port)).is_ok()
}

fn environment_data_dir(workspace_root: &Path, environment_id: &str) -> PathBuf {
    workspace_root
        .join(".studio")
        .join("environments")
        .join(environment_id)
}

fn spawn_localnet(
    acton_executable: &Path,
    workspace_root: &Path,
    db_path: PathBuf,
    config: &EnvironmentConfig,
) -> Result<Child, EnvironmentRuntimeError> {
    let mut command = Command::new(acton_executable);
    command
        .arg("--project-root")
        .arg(workspace_root)
        .arg("localnet")
        .arg("start")
        .arg("--port")
        .arg(config.port.to_string())
        .arg("--db-path")
        .arg(db_path)
        .current_dir(workspace_root)
        .stdin(Stdio::null())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .kill_on_drop(true);

    if let Some(fork_network) = &config.fork_network {
        command.arg("--fork-net").arg(fork_network);
    }
    if let Some(fork_block_number) = config.fork_block_number {
        command
            .arg("--fork-block-number")
            .arg(fork_block_number.to_string());
    }
    if !config.accounts.is_empty() {
        command.arg("--accounts").arg(config.accounts.join(","));
    }
    if let Some(rate_limit) = config.rate_limit {
        command.arg("--rate-limit").arg(rate_limit.to_string());
    }
    if let Some(response_delay_ms) = config.response_delay_ms {
        command
            .arg("--response-delay-ms")
            .arg(response_delay_ms.to_string());
    }
    if let Some(block_interval_ms) = config.block_interval_ms {
        command
            .arg("--block-interval-ms")
            .arg(block_interval_ms.to_string());
    }
    if config.no_mining {
        command.arg("--no-mining");
    }
    if config.mine_empty_blocks {
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

async fn monitor_localnet(
    runtime: Arc<LocalProcessRuntimeInner>,
    environment: Arc<LocalEnvironment>,
    generation: u64,
) {
    let port = environment.details.read().await.config.port;
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
    let mut next_register_retry = Instant::now();

    loop {
        if runtime.shutting_down.load(Ordering::Acquire) {
            return;
        }
        if !has_running_environment(runtime).await {
            runtime.artifact_coordinator_wakeup.notified().await;
            failed_build_fingerprint = None;
            next_register_retry = Instant::now();
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
            && has_running_environment(runtime).await
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
                            next_register_retry = Instant::now();
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

        if state.current.is_some() && Instant::now() >= next_register_retry {
            let had_failures = publish_current_artifact_revision(runtime, &mut state).await;
            next_register_retry = if had_failures {
                Instant::now() + PROJECT_ARTIFACT_REGISTER_RETRY_INTERVAL
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
    let environments = runtime.environments.read().await.clone();
    for environment in environments {
        let details = environment.details.read().await.clone();
        let generation = environment.generation.load(Ordering::Acquire);
        if details.status != EnvironmentStatus::Running
            || !state.needs_publish(&details.id, generation)
        {
            continue;
        }
        let result = {
            let revision = state.current.as_ref().expect("revision exists");
            runtime
                .artifact_synchronizer
                .register(&details.rpc_url, &revision.artifacts)
                .await
        };
        match result {
            Ok(()) => {
                state.mark_published(details.id.clone(), generation, revision_id);
                tracing::debug!(
                    environment_id = %details.id,
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
                    environment_id = %details.id,
                    %error,
                    "Failed to register Acton project artifacts"
                );
            }
        }
    }
    had_failures
}

async fn has_running_environment(runtime: &LocalProcessRuntimeInner) -> bool {
    let environments = runtime.environments.read().await.clone();
    for environment in environments {
        if environment.details.read().await.status == EnvironmentStatus::Running {
            return true;
        }
    }
    false
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

async fn stop_environment(environment: &LocalEnvironment) {
    let _lifecycle_guard = environment.lifecycle.lock().await;
    let current_status = environment.details.read().await.status;
    if current_status == EnvironmentStatus::Stopped {
        return;
    }

    environment.generation.fetch_add(1, Ordering::AcqRel);
    set_environment_status(environment, EnvironmentStatus::Stopping, None).await;
    terminate_child(environment).await;
    set_environment_status(environment, EnvironmentStatus::Stopped, None).await;
}

async fn restart_environment(
    runtime: &Arc<LocalProcessRuntimeInner>,
    environment: &Arc<LocalEnvironment>,
) -> Result<StudioEnvironment, EnvironmentRuntimeError> {
    let _lifecycle_guard = environment.lifecycle.lock().await;
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

    environment.generation.fetch_add(1, Ordering::AcqRel);
    terminate_child(environment).await;
    select_port(Some(details.config.port))?;
    let child = spawn_localnet(
        &runtime.acton_executable,
        &runtime.workspace_root,
        environment_data_dir(&runtime.workspace_root, &details.id).join("localnet.sqlite"),
        &details.config,
    )?;
    *environment.child.lock().await = Some(child);
    let generation = environment.generation.load(Ordering::Acquire);
    set_environment_status(environment, EnvironmentStatus::Starting, None).await;
    tokio::spawn(monitor_localnet(
        Arc::clone(runtime),
        Arc::clone(environment),
        generation,
    ));
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
mod artifact_coordinator_tests {
    use std::fs;

    use serde_json::json;
    use tempfile::tempdir;

    use super::{ArtifactCoordinatorState, ProjectArtifactSynchronizer, SourceRegistration};

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
            vec![SourceRegistration {
                code_hash: "stale".to_owned(),
                source: json!({"revision": "stale"}),
            }],
        ));
        assert!(state.current.is_none());

        assert!(state.commit_if_stable(
            after_build.clone(),
            &after_build,
            vec![SourceRegistration {
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
            vec![SourceRegistration {
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
            SourceRegistration {
                code_hash: "old-code".to_owned(),
                source: json!({"bundle": "old"}),
            },
            SourceRegistration {
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
