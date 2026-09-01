//! Typed adapter for the official `validator-engine` executable.
//!
//! The same release-specific command line has three different meanings in
//! Localton: create a fresh database, expose a temporary control console during
//! identity bootstrap, and run a persistent node. Keeping those meanings as
//! separate methods prevents workflow code from switching a large flag set with
//! a boolean and keeps process restart policy outside this adapter.

use std::{
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

use anyhow::{Result, ensure};
use async_trait::async_trait;
use tokio::process::Command;
use tracing::{info, warn};

use crate::{
    runtime::{ManagedProcess, ServiceHandle, run_checked},
    storage::{NodeLayout, NodeSettings},
};

use super::{
    types::{AdnlEndpoint, KeyId, OperationContext},
    validator_engine_config::ValidatorEngineConfig,
};

const TOOL_NAME: &str = "validator-engine";

/// Paths used by both TON's own logger and Localton's process supervisor.
///
/// `engine` is passed through `--logname`, while `stdout` and `stderr` are owned
/// by [`ManagedProcess`]. Keeping all three explicit matters because TON emits
/// useful diagnostics through both channels and because log routing is a runtime
/// concern, not something the bootstrap workflow should reconstruct implicitly.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatorLogPaths {
    pub engine: PathBuf,
    pub stdout: PathBuf,
    pub stderr: PathBuf,
}

/// Database created and subsequently reopened by `validator-engine`.
///
/// A value of this type means the generated `config.json` was observed and
/// validated by the adapter. It does not mean the node is ready, synchronized,
/// or configured with validator keys; those are later workflow postconditions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatorDatabase {
    pub path: PathBuf,
    pub config: PathBuf,
}

impl ValidatorDatabase {
    /// Describes a database path before an initialization operation creates it.
    ///
    /// Persisted state must enter through [`Self::open`], which proves the
    /// generated config can be parsed before a long-running process is spawned.
    #[must_use]
    pub fn at(path: impl Into<PathBuf>) -> Self {
        let path = path.into();
        let config = path.join("config.json");
        Self { path, config }
    }

    /// Opens an existing engine database and validates its complete typed config.
    ///
    /// Persistent startup receives this value instead of a raw path, so it does
    /// not need to rediscover or reparse `config.json` immediately before spawn.
    pub fn open(path: impl Into<PathBuf>) -> Result<Self> {
        let database = Self::at(path);
        OfficialValidatorEngine::validate_database(&database)?;
        Ok(database)
    }

    /// Adds host-local control and optional liteserver identities to this database.
    pub fn install_control_and_liteserver(
        &self,
        node: &NodeSettings,
        control_server: KeyId,
        control_client: KeyId,
        liteserver: Option<KeyId>,
    ) -> Result<()> {
        ensure!(
            node.liteserver == liteserver.is_some(),
            "node `{}` liteserver key does not match its service settings",
            node.name
        );
        for id in std::iter::once(control_server).chain(liteserver) {
            let path = self.private_key_path(id);
            ensure!(
                path.is_file(),
                "validator service key {id} is missing from {}",
                path.display()
            );
        }
        let mut config = ValidatorEngineConfig::load(&self.config)?;
        config.set_local_services(
            node.console_port,
            control_server,
            control_client,
            liteserver.map(|id| (node.liteserver_port, id)),
        );
        config.save(&self.config)
    }

    /// Resolves one engine-owned identity to its canonical keyring filename.
    pub fn private_key_path(&self, id: KeyId) -> PathBuf {
        self.path.join("keyring").join(id.to_keyring_filename())
    }
}

/// Inputs for the one-shot database creation mode.
///
/// This operation is intentionally not declared idempotent. With the pinned TON
/// release the same command exits after creating a fresh database, but reopens an
/// existing database as a long-running node. A failed attempt must therefore be
/// recovered by the genesis workflow, which owns cleanup and the manifest commit
/// boundary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatorInitializeRequest {
    pub global_config: PathBuf,
    pub database: PathBuf,
    pub log_path: PathBuf,
    pub endpoint: AdnlEndpoint,
    pub out_port: u16,
    pub threads: u16,
    pub verbosity: u8,
}

impl ValidatorInitializeRequest {
    /// Derives every release-level initialization input from one managed node.
    ///
    /// Centralizing path conventions here keeps bootstrap focused on operation
    /// order and prevents genesis and joined nodes from constructing different
    /// validator-engine command inputs.
    pub fn for_node(node_layout: &NodeLayout, node: &NodeSettings) -> Self {
        Self {
            global_config: node_layout.global_config.clone(),
            database: node_layout.db.clone(),
            log_path: node_layout.logs.join("validator-init"),
            endpoint: AdnlEndpoint::new(node.public_ip, node.adnl_port),
            out_port: node.out_port,
            threads: node.threads,
            verbosity: node.verbosity,
        }
    }
}

/// Inputs for the temporary engine used while identities are registered.
///
/// Starting the process has no readiness guarantee. The caller must probe the
/// authenticated console, perform mutations in the required order, stop the
/// handle on every exit path, and decide whether an identity transition warrants
/// a restart. The adapter never retries or restarts this service.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatorBootstrapRequest {
    pub node_name: String,
    pub global_config: PathBuf,
    pub database: ValidatorDatabase,
    pub logs: ValidatorLogPaths,
    pub endpoint: AdnlEndpoint,
    pub threads: u16,
    pub verbosity: u8,
}

/// Retention and synchronization flags used only by a persistent node.
///
/// These fields map to release-specific `validator-engine` switches. They remain
/// grouped so bootstrap mode cannot accidentally inherit archival policy and so
/// a TON release upgrade has one obvious compatibility surface to review.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ValidatorRetentionPolicy {
    pub sync_before_seconds: u64,
    pub state_ttl_seconds: u64,
    pub block_ttl_seconds: u64,
    pub archive_ttl_seconds: u64,
    pub key_proof_ttl_seconds: u64,
}

/// Inputs for a supervised, persistent full node or validator.
///
/// Reopening a complete database is restart-safe, but readiness and liveness are
/// still workflow responsibilities. Success only means that the child process was
/// spawned and is now owned by the returned [`ServiceHandle`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatorStartRequest {
    pub node_name: String,
    pub global_config: PathBuf,
    pub database: ValidatorDatabase,
    pub logs: ValidatorLogPaths,
    pub endpoint: AdnlEndpoint,
    pub threads: u16,
    pub verbosity: u8,
    pub retention: ValidatorRetentionPolicy,
    pub initial_sync_delay: Duration,
}

/// Semantic contract implemented by a `validator-engine` provider.
///
/// The trait deliberately excludes console commands, election policy, readiness
/// polling, retries, and restarts. Those need multi-step workflow knowledge and
/// would be unsafe to hide inside a binary adapter. Implementations may replace
/// the official process in tests as long as they preserve the artifact and
/// service-lifecycle postconditions described by each method.
#[async_trait]
pub trait ValidatorEngine: Send + Sync {
    /// Creates and validates a fresh engine database, including Localton's selected
    /// outbound ADNL port.
    ///
    /// The operation is one-shot and bounded by `context.timeout`. It is not safe
    /// to retry against the same partially initialized directory without a
    /// workflow-owned recovery step.
    async fn initialize(
        &self,
        context: &OperationContext,
        request: ValidatorInitializeRequest,
    ) -> Result<ValidatorDatabase>;

    /// Starts the temporary engine used to expose the control console.
    ///
    /// The returned handle owns process termination. No console readiness or
    /// automatic restart is implied by a successful return.
    async fn start_bootstrap(&self, request: ValidatorBootstrapRequest) -> Result<ServiceHandle>;

    /// Starts a persistent engine with explicit synchronization and retention
    /// policy.
    ///
    /// The caller must register the handle with the network supervisor and prove
    /// chain readiness independently.
    async fn start_persistent(&self, request: ValidatorStartRequest) -> Result<ServiceHandle>;
}

/// Production adapter for the pinned official `validator-engine` program.
///
/// It owns only the executable contract of that release: argv rendering,
/// one-shot timeout enforcement, generated-config validation, the narrow
/// `out_port` patch, and conversion of a spawned process into a service handle.
#[derive(Debug, Clone)]
pub struct OfficialValidatorEngine {
    executable: PathBuf,
}

impl OfficialValidatorEngine {
    /// Binds the adapter to an executable from an already validated TON
    /// distribution. Distribution resolution remains outside the adapter so all
    /// tools in a [`TonToolchain`](crate::ton::toolchain::Toolchain) use one pinned
    /// release.
    #[must_use]
    pub fn new(executable: impl Into<PathBuf>) -> Self {
        Self {
            executable: executable.into(),
        }
    }

    /// Builds the common command used by all three engine modes.
    ///
    /// In TON v2026.06 the trailing `false` and `--ip` are positional/late options
    /// and must stay after the cell database flags. Initialization and temporary
    /// bootstrap intentionally render the same argv: the existing database state
    /// determines whether this release exits after creation or keeps running.
    fn base_command(
        &self,
        global_config: &Path,
        database: &Path,
        engine_log: &Path,
        threads: u16,
        verbosity: u8,
    ) -> Command {
        let mut command = Command::new(&self.executable);
        command
            .args([
                "--verbosity",
                &verbosity.to_string(),
                "--threads",
                &threads.to_string(),
                "--global-config",
            ])
            .arg(global_config)
            .arg("--db")
            .arg(database)
            .arg("--logname")
            .arg(engine_log)
            .args(["--session-logs", "", "--celldb-preload-all"]);
        command
    }

    /// Appends the release-specific tail after optional persistent flags.
    ///
    /// Keeping this separate makes it impossible for persistent options to be
    /// appended after `--ip`, which the official CLI does not interpret as the
    /// same mode.
    fn finish_command(mut command: Command, endpoint: AdnlEndpoint) -> Command {
        command.arg("false").args(["--ip", &endpoint.to_string()]);
        command
    }

    /// Verifies that a database can be reopened before any long-running process
    /// is spawned. This catches a missing or moved generated config synchronously
    /// instead of reporting an opaque early child exit through supervision.
    fn validate_database(database: &ValidatorDatabase) -> Result<()> {
        ensure!(
            database.path.is_dir(),
            "validator database does not exist: {}",
            database.path.display()
        );
        ensure!(
            database.config.is_file(),
            "validator config does not exist: {}",
            database.config.display()
        );
        let config = ValidatorEngineConfig::load(&database.config)?;
        for id in config.private_key_ids() {
            let key = database.private_key_path(id);
            ensure!(
                key.is_file(),
                "validator config references missing private key {id}: {}",
                key.display()
            );
        }
        Ok(())
    }
}

#[async_trait]
impl ValidatorEngine for OfficialValidatorEngine {
    async fn initialize(
        &self,
        context: &OperationContext,
        request: ValidatorInitializeRequest,
    ) -> Result<ValidatorDatabase> {
        let started = Instant::now();
        trace_progress(context.node_name.as_deref(), "initialize", "starting");
        let result = async {
            ensure!(
                request.threads > 0,
                "validator-engine threads must be positive"
            );
            request
                .endpoint
                .ensure_available("validator-engine initialization")?;
            let command = Self::finish_command(
                self.base_command(
                    &request.global_config,
                    &request.database,
                    &request.log_path,
                    request.threads,
                    request.verbosity,
                ),
                request.endpoint,
            );
            trace_progress(context.node_name.as_deref(), "initialize", "executing");
            let output =
                run_checked("validator-engine initialization", command, context.timeout).await?;
            if !output.stderr.trim().is_empty() {
                warn!(
                    "ton.tool" = TOOL_NAME,
                    operation = "initialize",
                    node = context.node_name.as_deref().unwrap_or("unknown"),
                    stderr = output.stderr.trim(),
                    "validator initialization wrote diagnostics"
                );
            }

            let database = ValidatorDatabase::at(request.database);
            ensure!(
                database.config.is_file(),
                "validator initialization did not create {}",
                database.config.display()
            );
            let mut config = ValidatorEngineConfig::load(&database.config)?;
            config.set_out_port(request.out_port);
            config.save(&database.config)?;
            Self::validate_database(&database)?;
            trace_progress(
                context.node_name.as_deref(),
                "initialize",
                "database_validated",
            );
            Ok(database)
        }
        .await;
        trace_outcome(context.node_name.as_deref(), "initialize", started, &result);
        result
    }

    async fn start_bootstrap(&self, request: ValidatorBootstrapRequest) -> Result<ServiceHandle> {
        let started = Instant::now();
        trace_progress(
            Some(&request.node_name),
            "start_bootstrap",
            "validating_database",
        );
        let result = (|| {
            ensure!(
                request.threads > 0,
                "validator-engine threads must be positive"
            );
            Self::validate_database(&request.database)?;
            request
                .endpoint
                .ensure_available("temporary validator-engine")?;
            let command = Self::finish_command(
                self.base_command(
                    &request.global_config,
                    &request.database.path,
                    &request.logs.engine,
                    request.threads,
                    request.verbosity,
                ),
                request.endpoint,
            );
            trace_progress(Some(&request.node_name), "start_bootstrap", "spawning");
            ManagedProcess::spawn(
                format!("{} temporary validator-engine", request.node_name),
                command,
                &request.logs.stdout,
                &request.logs.stderr,
            )
            .map(ServiceHandle::from)
        })();
        trace_outcome(
            Some(&request.node_name),
            "start_bootstrap",
            started,
            &result,
        );
        result
    }

    async fn start_persistent(&self, request: ValidatorStartRequest) -> Result<ServiceHandle> {
        let started = Instant::now();
        trace_progress(
            Some(&request.node_name),
            "start_persistent",
            "validating_database",
        );
        let result = (|| {
            ensure!(
                request.threads > 0,
                "validator-engine threads must be positive"
            );
            request.endpoint.ensure_available("validator-engine")?;
            let mut command = self.base_command(
                &request.global_config,
                &request.database.path,
                &request.logs.engine,
                request.threads,
                request.verbosity,
            );
            command.args([
                "--initial-sync-delay",
                &duration_seconds(request.initial_sync_delay),
                "--sync-before",
                &request.retention.sync_before_seconds.to_string(),
                "--state-ttl",
                &request.retention.state_ttl_seconds.to_string(),
                "--block-ttl",
                &request.retention.block_ttl_seconds.to_string(),
                "--archive-ttl",
                &request.retention.archive_ttl_seconds.to_string(),
                "--key-proof-ttl",
                &request.retention.key_proof_ttl_seconds.to_string(),
            ]);
            let command = Self::finish_command(command, request.endpoint);
            trace_progress(Some(&request.node_name), "start_persistent", "spawning");
            ManagedProcess::spawn(
                request.node_name.clone(),
                command,
                &request.logs.stdout,
                &request.logs.stderr,
            )
            .map(ServiceHandle::from)
        })();
        trace_outcome(
            Some(&request.node_name),
            "start_persistent",
            started,
            &result,
        );
        result
    }
}

/// Formats a CLI duration without losing sub-second policy values.
///
/// Whole seconds retain a `.0` suffix to preserve the pinned release's existing
/// command snapshot (`0.0` for immediate synchronization) while non-whole values
/// use Rust's shortest lossless floating-point representation.
fn duration_seconds(duration: Duration) -> String {
    if duration.subsec_nanos() == 0 {
        format!("{}.0", duration.as_secs())
    } else {
        duration.as_secs_f64().to_string()
    }
}

/// Emits a redacted progress event for an engine lifecycle transition.
fn trace_progress(node: Option<&str>, operation: &'static str, progress: &'static str) {
    info!(
        "ton.tool" = TOOL_NAME,
        operation,
        node = node.unwrap_or("unknown"),
        duration_ms = 0_u128,
        outcome = "pending",
        progress,
        "TON tool operation progress"
    );
}

/// Emits the terminal semantic outcome without serializing request paths, key
/// material, or release argv into structured telemetry.
fn trace_outcome<T>(
    node: Option<&str>,
    operation: &'static str,
    started: Instant,
    result: &Result<T>,
) {
    let duration_ms = started.elapsed().as_millis();
    match result {
        Ok(_) => info!(
            "ton.tool" = TOOL_NAME,
            operation,
            node = node.unwrap_or("unknown"),
            duration_ms,
            outcome = "success",
            progress = "complete",
            "TON tool operation completed"
        ),
        Err(error) => warn!(
            "ton.tool" = TOOL_NAME,
            operation,
            node = node.unwrap_or("unknown"),
            duration_ms,
            outcome = "failure",
            progress = "complete",
            error = %error,
            "TON tool operation failed"
        ),
    }
}

#[cfg(test)]
mod tests {
    use std::{ffi::OsStr, net::Ipv4Addr};

    use super::*;

    fn args(command: &Command) -> Vec<String> {
        command
            .as_std()
            .get_args()
            .map(OsStr::to_string_lossy)
            .map(|value| value.into_owned())
            .collect()
    }

    fn endpoint() -> AdnlEndpoint {
        AdnlEndpoint {
            ip: Ipv4Addr::new(192, 168, 27, 8),
            port: 18_006,
        }
    }

    #[test]
    fn bootstrap_command_matches_pinned_release_snapshot() {
        let adapter = OfficialValidatorEngine::new("/ton/validator-engine");
        let command = OfficialValidatorEngine::finish_command(
            adapter.base_command(
                Path::new("/state/global.config.json"),
                Path::new("/state/db"),
                Path::new("/state/logs/validator-init"),
                4,
                2,
            ),
            endpoint(),
        );

        assert_eq!(
            args(&command),
            [
                "--verbosity",
                "2",
                "--threads",
                "4",
                "--global-config",
                "/state/global.config.json",
                "--db",
                "/state/db",
                "--logname",
                "/state/logs/validator-init",
                "--session-logs",
                "",
                "--celldb-preload-all",
                "false",
                "--ip",
                "192.168.27.8:18006",
            ]
        );
    }

    #[test]
    fn persistent_command_matches_pinned_release_snapshot() {
        let adapter = OfficialValidatorEngine::new("/ton/validator-engine");
        let mut command = adapter.base_command(
            Path::new("/state/global.config.json"),
            Path::new("/state/db"),
            Path::new("/state/logs/validator-engine"),
            8,
            1,
        );
        command.args([
            "--initial-sync-delay",
            &duration_seconds(Duration::ZERO),
            "--sync-before",
            "3600",
            "--state-ttl",
            "31536000",
            "--block-ttl",
            "31536000",
            "--archive-ttl",
            "31536000",
            "--key-proof-ttl",
            "315360000",
        ]);
        let command = OfficialValidatorEngine::finish_command(command, endpoint());

        assert_eq!(
            args(&command),
            [
                "--verbosity",
                "1",
                "--threads",
                "8",
                "--global-config",
                "/state/global.config.json",
                "--db",
                "/state/db",
                "--logname",
                "/state/logs/validator-engine",
                "--session-logs",
                "",
                "--celldb-preload-all",
                "--initial-sync-delay",
                "0.0",
                "--sync-before",
                "3600",
                "--state-ttl",
                "31536000",
                "--block-ttl",
                "31536000",
                "--archive-ttl",
                "31536000",
                "--key-proof-ttl",
                "315360000",
                "false",
                "--ip",
                "192.168.27.8:18006",
            ]
        );
    }
}
