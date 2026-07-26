use std::net::{IpAddr, Ipv4Addr, SocketAddr, TcpListener as StdTcpListener};
use std::path::{Path, PathBuf};
use std::process::{ExitStatus, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use tokio::net::TcpStream;
use tokio::process::{Child, Command};
use tokio::sync::{Mutex, RwLock};
use tokio::time::{Instant, sleep, timeout};

use crate::environment::{
    CreateEnvironmentRequest, EnvironmentConfig, EnvironmentRuntime, EnvironmentRuntimeError,
    EnvironmentRuntimeFuture, EnvironmentStatus, StudioEnvironment,
};

const FIRST_LOCALNET_PORT: u16 = 5411;
const LOCALNET_READY_TIMEOUT: Duration = Duration::from_secs(15);
const LOCALNET_READY_POLL_INTERVAL: Duration = Duration::from_millis(100);
const LOCALNET_STATUS_POLL_INTERVAL: Duration = Duration::from_millis(500);

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
}

struct LocalEnvironment {
    details: RwLock<StudioEnvironment>,
    child: Mutex<Option<Child>>,
    lifecycle: Mutex<()>,
    generation: AtomicU64,
}

impl LocalProcessEnvironmentRuntime {
    #[must_use]
    pub fn new(acton_executable: impl Into<PathBuf>, workspace_root: impl Into<PathBuf>) -> Self {
        Self {
            inner: Arc::new(LocalProcessRuntimeInner {
                acton_executable: acton_executable.into(),
                workspace_root: workspace_root.into(),
                next_id: AtomicU64::new(1),
                create_lock: Mutex::new(()),
                environments: RwLock::new(Vec::new()),
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
            tokio::spawn(monitor_localnet(environment, 1));
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
    request.name = request.name.trim().to_owned();
    if request.name.is_empty() {
        return Err(EnvironmentRuntimeError::InvalidRequest {
            code: "environment_name_required",
            message: "Environment name is required".to_owned(),
        });
    }
    if request.name.chars().count() > 80 {
        return Err(EnvironmentRuntimeError::InvalidRequest {
            code: "environment_name_too_long",
            message: "Environment name must contain at most 80 characters".to_owned(),
        });
    }
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

async fn monitor_localnet(environment: Arc<LocalEnvironment>, generation: u64) {
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
    runtime: &LocalProcessRuntimeInner,
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
    tokio::spawn(monitor_localnet(Arc::clone(environment), generation));
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
