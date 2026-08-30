//! Typed interface to the official `dht-server` program.
//!
//! Initialization creates a durable DHT identity and exits, while normal startup
//! reopens that identity as a supervised long-running service. Separate request
//! types make this lifecycle distinction visible and prevent a routine restart
//! from silently replacing the keyring already published in global config.

use std::{path::PathBuf, time::Instant};

use anyhow::{Result, ensure};
use async_trait::async_trait;
use tokio::process::Command;
use tracing::{Instrument, Span, field::Empty, info, info_span, warn};

use crate::{
    binaries::TonBinaries,
    runtime::{ManagedProcess, ServiceHandle, run_checked},
};

use super::{
    types::{AdnlEndpoint, DhtDatabase, OperationContext},
    validator_engine_config::ValidatorEngineConfig,
};

/// Inputs for the one-shot DHT database and identity creation phase.
///
/// `out_port` is separate from the ADNL endpoint: it controls the binary-owned
/// outbound socket policy patched into `config.json`, whereas `endpoint` is the
/// address on which peers reach this DHT node.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DhtInitializeRequest {
    /// Preliminary or final TON global configuration read during initialization.
    pub global_config: PathBuf,
    /// Destination directory for config and keyring artifacts.
    pub database: PathBuf,
    /// Native TON log prefix used by the initialization process.
    pub log_path: PathBuf,
    /// IPv4/UDP endpoint assigned to the DHT ADNL identity.
    pub endpoint: AdnlEndpoint,
    /// Instance-selected outbound UDP port stored in binary-owned config.
    pub out_port: u16,
    /// Worker thread count passed to the pinned `dht-server` release.
    pub threads: usize,
    /// Official TON logging verbosity.
    pub verbosity: u8,
}

/// Inputs for reopening an initialized DHT database as a supervised service.
///
/// The three log paths have distinct owners: `log_path` belongs to TON's own
/// logger, while stdout and stderr are append-only process-supervisor artifacts.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DhtStartRequest {
    /// Final global configuration containing the signed DHT descriptors.
    pub global_config: PathBuf,
    /// Existing database whose identity must remain stable across restarts.
    pub database: DhtDatabase,
    /// Native TON service log prefix passed through `-l`.
    pub log_path: PathBuf,
    /// Append-only stdout log managed by Localton.
    pub stdout_log: PathBuf,
    /// Append-only stderr log managed by Localton.
    pub stderr_log: PathBuf,
    /// Public IPv4/UDP endpoint reopened by the service.
    pub endpoint: AdnlEndpoint,
    /// Worker thread count passed to the pinned `dht-server` release.
    pub threads: usize,
    /// Official TON logging verbosity.
    pub verbosity: u8,
}

/// Semantic operations Localton requires from `dht-server`.
///
/// Implementations own CLI flags, subprocess output, and artifact validation.
/// Workflows retain ownership of the global-config/DHT-descriptor cycle and the
/// registry into which the returned service handle is inserted.
#[async_trait]
pub trait DhtServer: Send + Sync {
    /// Creates a persistent DHT database and returns its validated identity files.
    async fn initialize(
        &self,
        context: &OperationContext,
        request: DhtInitializeRequest,
    ) -> Result<DhtDatabase>;

    /// Reopens a previously initialized database without regenerating its identity.
    ///
    /// The context timeout is diagnostic only because the returned service is
    /// intentionally long-running; its lifecycle is controlled through
    /// [`ServiceHandle`] rather than a one-shot deadline.
    async fn start(
        &self,
        context: &OperationContext,
        request: DhtStartRequest,
    ) -> Result<ServiceHandle>;
}

/// Production DHT adapter backed by the pinned official TON distribution.
///
/// This type is the version-compatibility surface for initialization and persistent
/// flags. It narrowly patches `out_port` while preserving every unknown field in
/// the binary-owned configuration.
#[derive(Clone, Debug)]
pub struct OfficialDhtServer {
    binaries: TonBinaries,
}

impl OfficialDhtServer {
    /// Binds the adapter to an already validated official TON distribution.
    pub fn new(binaries: TonBinaries) -> Self {
        Self { binaries }
    }

    /// Builds the pinned release's one-shot initialization command.
    ///
    /// Keeping this renderer private ensures workflows cannot reuse it as a raw
    /// command escape hatch. Tests cover it as the adapter's version contract.
    fn initialize_command(&self, request: &DhtInitializeRequest) -> Command {
        let mut command = Command::new(self.binaries.command("dht-server"));
        command
            .args([
                "--verbosity",
                &request.verbosity.to_string(),
                "--threads",
                &request.threads.to_string(),
                "--global-config",
            ])
            .arg(&request.global_config)
            .arg("--logname")
            .arg(&request.log_path)
            .arg("--db")
            .arg(&request.database)
            .args(["-I", &request.endpoint.to_string()]);
        command
    }

    /// Builds the pinned release's persistent DHT service command.
    ///
    /// Persistent flags intentionally differ from initialization flags. This
    /// renderer never creates or edits identity artifacts.
    fn start_command(&self, request: &DhtStartRequest) -> Command {
        let mut command = Command::new(self.binaries.command("dht-server"));
        command
            .args([
                "-v",
                &request.verbosity.to_string(),
                "-t",
                &request.threads.to_string(),
                "-C",
            ])
            .arg(&request.global_config)
            .arg("-l")
            .arg(&request.log_path)
            .arg("-D")
            .arg(&request.database.path)
            .args(["-I", &request.endpoint.to_string()]);
        command
    }
}

#[async_trait]
impl DhtServer for OfficialDhtServer {
    async fn initialize(
        &self,
        context: &OperationContext,
        request: DhtInitializeRequest,
    ) -> Result<DhtDatabase> {
        let started = Instant::now();
        let span = operation_span(context, "initialize");
        let result = async {
            request
                .endpoint
                .ensure_available("dht-server initialization")?;
            info!(
                milestone = "database_initialization_started",
                database_path = %request.database.display(),
                config_path = %request.database.join("config.json").display(),
                ton_log_path = %request.log_path.display(),
                endpoint = %request.endpoint,
                "initializing TON DHT database"
            );
            run_checked(
                "dht-server initialization",
                self.initialize_command(&request),
                context.timeout,
            )
            .await?;

            let config_path = request.database.join("config.json");
            ensure!(
                config_path.is_file(),
                "DHT initialization did not create {}",
                config_path.display()
            );
            info!(
                milestone = "binary_artifacts_created",
                database_path = %request.database.display(),
                config_path = %config_path.display(),
                "dht-server created its database"
            );

            // `dht-server` emits the shared engine config schema. Parse the whole
            // document before changing Localton's outbound port so release drift
            // fails here instead of surfacing as an opaque startup error.
            let mut config = ValidatorEngineConfig::load(&config_path)?;
            config.set_out_port(request.out_port);
            config.save(&config_path)?;
            let database = DhtDatabase::open(request.database)?;
            info!(
                milestone = "database_validated",
                database_path = %database.path.display(),
                config_path = %database.config.display(),
                keyring_path = %database.keyring_dir().display(),
                key_count = database.keyring.len(),
                out_port = request.out_port,
                "TON DHT database is ready"
            );
            Ok(database)
        }
        .instrument(span.clone())
        .await;
        finish_operation(&span, started, &result);
        result
    }

    async fn start(
        &self,
        context: &OperationContext,
        request: DhtStartRequest,
    ) -> Result<ServiceHandle> {
        let started = Instant::now();
        let span = operation_span(context, "start");
        let result = async {
            request.endpoint.ensure_available("dht-server")?;
            info!(
                milestone = "service_spawn_started",
                database_path = %request.database.path.display(),
                config_path = %request.database.config.display(),
                ton_log_path = %request.log_path.display(),
                stdout_log_path = %request.stdout_log.display(),
                stderr_log_path = %request.stderr_log.display(),
                endpoint = %request.endpoint,
                "starting persistent TON DHT service"
            );
            let process = ManagedProcess::spawn(
                "dht",
                self.start_command(&request),
                &request.stdout_log,
                &request.stderr_log,
            )?;
            let pid = process.id();
            info!(
                milestone = "service_spawned",
                ?pid,
                database_path = %request.database.path.display(),
                stdout_log_path = %request.stdout_log.display(),
                stderr_log_path = %request.stderr_log.display(),
                "persistent TON DHT service started"
            );
            Ok(ServiceHandle::from(process))
        }
        .instrument(span.clone())
        .await;
        finish_operation(&span, started, &result);
        result
    }
}

/// Starts the structured envelope shared by both phases of the DHT lifecycle.
///
/// Artifact paths are emitted as progress events, while the span keeps stable
/// low-cardinality fields suitable for aggregating tool latency and failures.
fn operation_span(context: &OperationContext, operation: &'static str) -> Span {
    info_span!(
        "ton_tool_operation",
        ton.tool = "dht-server",
        operation,
        node = context.node_name.as_deref().unwrap_or("network"),
        outcome = Empty,
        duration_ms = Empty,
    )
}

/// Records the semantic outcome, including post-process artifact validation rather
/// than treating a zero subprocess exit status as sufficient success.
fn finish_operation<T>(span: &Span, started: Instant, result: &Result<T>) {
    let duration_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
    let outcome = if result.is_ok() { "success" } else { "error" };
    span.record("duration_ms", duration_ms);
    span.record("outcome", outcome);
    span.in_scope(|| match result {
        Ok(_) => info!(duration_ms, outcome, "TON tool operation completed"),
        Err(error) => warn!(duration_ms, outcome, %error, "TON tool operation failed"),
    });
}

#[cfg(test)]
mod tests {
    use std::{ffi::OsStr, net::Ipv4Addr};

    use super::*;

    fn adapter() -> OfficialDhtServer {
        OfficialDhtServer::new(TonBinaries {
            root: PathBuf::from("/ton"),
        })
    }

    #[test]
    fn renders_official_initialization_arguments() {
        let command = adapter().initialize_command(&DhtInitializeRequest {
            global_config: PathBuf::from("/state/global.config.json"),
            database: PathBuf::from("/state/dht"),
            log_path: PathBuf::from("/state/logs/dht-init"),
            endpoint: AdnlEndpoint::new(Ipv4Addr::new(192, 168, 27, 4), 18_000),
            out_port: 32_777,
            threads: 4,
            verbosity: 3,
        });
        let args: Vec<_> = command.as_std().get_args().collect();

        assert_eq!(
            args,
            [
                OsStr::new("--verbosity"),
                OsStr::new("3"),
                OsStr::new("--threads"),
                OsStr::new("4"),
                OsStr::new("--global-config"),
                OsStr::new("/state/global.config.json"),
                OsStr::new("--logname"),
                OsStr::new("/state/logs/dht-init"),
                OsStr::new("--db"),
                OsStr::new("/state/dht"),
                OsStr::new("-I"),
                OsStr::new("192.168.27.4:18000"),
            ]
        );
    }

    #[test]
    fn renders_official_persistent_arguments() {
        let command = adapter().start_command(&DhtStartRequest {
            global_config: PathBuf::from("/state/global.config.json"),
            database: DhtDatabase {
                path: PathBuf::from("/state/dht"),
                config: PathBuf::from("/state/dht/config.json"),
                keyring: vec![PathBuf::from(
                    "/state/dht/keyring/0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
                )],
            },
            log_path: PathBuf::from("/state/logs/dht-engine"),
            stdout_log: PathBuf::from("/state/logs/dht.stdout.log"),
            stderr_log: PathBuf::from("/state/logs/dht.stderr.log"),
            endpoint: AdnlEndpoint::new(Ipv4Addr::new(192, 168, 27, 4), 18_000),
            threads: 4,
            verbosity: 3,
        });
        let args: Vec<_> = command.as_std().get_args().collect();

        assert_eq!(
            args,
            [
                OsStr::new("-v"),
                OsStr::new("3"),
                OsStr::new("-t"),
                OsStr::new("4"),
                OsStr::new("-C"),
                OsStr::new("/state/global.config.json"),
                OsStr::new("-l"),
                OsStr::new("/state/logs/dht-engine"),
                OsStr::new("-D"),
                OsStr::new("/state/dht"),
                OsStr::new("-I"),
                OsStr::new("192.168.27.4:18000"),
            ]
        );
    }
}
