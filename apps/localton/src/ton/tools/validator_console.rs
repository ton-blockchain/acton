//! Typed adapter for `validator-engine-console`.
//!
//! Console commands mutate security-sensitive engine state and differ in retry
//! safety. This module keeps raw `-rc` strings private, parses every result into a
//! semantic value, and deliberately leaves retry ordering and engine restarts to
//! the validator workflow.

use std::{
    collections::BTreeMap,
    fmt,
    path::PathBuf,
    process::Stdio,
    time::{Duration, Instant},
};

use anyhow::{Context, Result, bail, ensure};
use async_trait::async_trait;
use base64::{Engine as _, engine::general_purpose::STANDARD};
use regex::Regex;
use tokio::{process::Command, time::timeout};
use tracing::{info, warn};

use crate::{
    runtime::run_checked,
    storage::{InitialSyncProgress, InitialSyncStage, StateDownloadProgress},
};

use super::types::{KeyId, OperationContext, TonPublicKey};

const TOOL_NAME: &str = "validator-engine-console";
const DEFAULT_CONNECT_TIMEOUT: Duration = Duration::from_secs(10);

/// Authenticated control endpoint exposed by one running validator engine.
///
/// The client private key and server public key establish the console ADNL
/// identity; they are paths rather than key bytes so adapter errors and telemetry
/// never need to serialize secret material. The endpoint becomes temporarily
/// unavailable when the engine changes its full-node identity, which is handled
/// explicitly by [`ValidatorConsole::change_full_node_address`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatorConsoleEndpoint {
    pub address: std::net::SocketAddr,
    pub client_private_key: PathBuf,
    pub server_public_key: PathBuf,
}

/// Parsed `getstats` response used for readiness and synchronization decisions.
///
/// TON releases may add stats without changing the console protocol, so the
/// adapter preserves named values instead of freezing the complete release output
/// into a rigid struct. Readiness is still validated before this value is returned.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatorStats {
    connection_ready: bool,
    values: BTreeMap<String, String>,
}

/// Best synchronization signal exposed by one validator-engine `getstats` call.
///
/// The engine reports native initial-sync stages before it has a masterchain
/// handle, then switches to a block timestamp. Keeping that release-specific
/// transition here prevents orchestration code from combining raw stat names.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ValidatorSynchronization {
    /// The engine has a masterchain block and reports timestamp-based progress
    BlockTime { block_time: u64, target_time: u64 },
    /// The engine is still downloading or preparing its initial persistent state
    Initial(InitialSyncProgress),
    /// The console is ready but has not exposed a more specific sync signal yet
    WaitingForMasterchain,
}

impl ValidatorStats {
    /// Reports whether the console emitted its explicit `conn ready` marker.
    ///
    /// Some releases omit the marker after returning a complete `getstats` table;
    /// the presence of `unixtime` is also accepted as a successful readiness
    /// proof, so callers should normally rely on `health()` success itself.
    #[must_use]
    pub const fn connection_ready(&self) -> bool {
        self.connection_ready
    }

    /// Returns an unmodified named value for release-specific diagnostics.
    #[must_use]
    pub fn value(&self, name: &str) -> Option<&str> {
        self.values.get(name).map(String::as_str)
    }

    /// Parses a numeric stat while retaining its semantic name in any error.
    ///
    /// This keeps synchronization workflows free of console line parsing and
    /// makes a release output change fail at the adapter boundary.
    pub fn value_u64(&self, name: &str) -> Result<u64> {
        self.value(name)
            .with_context(|| format!("validator stats do not contain `{name}`"))?
            .parse()
            .with_context(|| format!("validator stat `{name}` is not a u64"))
    }

    /// Engine wall-clock time reported by the pinned release.
    pub fn unix_time(&self) -> Result<u64> {
        self.value_u64("unixtime")
    }

    /// Timestamp of the latest synchronized masterchain block, when one exists.
    ///
    /// The pinned engine intentionally omits this stat while it downloads its
    /// initial persistent state, so absence is a synchronization state rather than
    /// a malformed console response.
    pub fn masterchain_block_time(&self) -> Result<Option<u64>> {
        self.value("masterchainblocktime")
            .map(|value| {
                value
                    .parse()
                    .context("validator stat `masterchainblocktime` is not a u64")
            })
            .transpose()
    }

    /// Parses validator-engine's native progress before a masterchain head exists.
    #[must_use]
    pub fn initial_sync_progress(&self) -> Option<InitialSyncProgress> {
        let status = self.value("process.initial_sync")?;
        let (stage, masterchain_seqno) = if let Some(value) =
            status.strip_prefix("starting, init block seqno ")
        {
            (InitialSyncStage::Starting, leading_u32(value))
        } else if let Some(value) = status.strip_prefix("last key block is ") {
            (InitialSyncStage::DiscoveringKeyBlocks, leading_u32(value))
        } else if let Some(value) = status.strip_prefix("downloading masterchain state ") {
            (
                InitialSyncStage::DownloadingMasterchainState,
                leading_u32(value),
            )
        } else if let Some(value) = status.strip_prefix("downloading all shard states, mc seqno ") {
            (InitialSyncStage::DownloadingShardStates, leading_u32(value))
        } else {
            (InitialSyncStage::Preparing, None)
        };

        // Part counters and network transfer estimates are separate stats and may
        // appear later than the high-level initial-sync stage.
        let (current_part, total_parts) = self
            .value("process.download_state")
            .and_then(state_part_progress)
            .map_or((None, None), |(current, total)| {
                (Some(current), Some(total))
            });
        let state_download = self
            .value("process.download_state_net")
            .and_then(state_download_progress);

        Some(InitialSyncProgress {
            stage,
            masterchain_seqno,
            current_part,
            total_parts,
            state_download,
        })
    }

    /// Selects the most precise synchronization signal currently available.
    pub fn synchronization(&self) -> Result<ValidatorSynchronization> {
        let target_time = self.unix_time()?;

        if let Some(block_time) = self.masterchain_block_time()?.filter(|time| *time > 0) {
            return Ok(ValidatorSynchronization::BlockTime {
                block_time,
                target_time,
            });
        }

        Ok(self.initial_sync_progress().map_or(
            ValidatorSynchronization::WaitingForMasterchain,
            ValidatorSynchronization::Initial,
        ))
    }
}

/// Ed25519 signature returned by the engine-owned validator key.
///
/// The encoded value is validated to contain exactly 64 bytes. Its `Debug`
/// implementation is redacted so request failures and structured traces cannot
/// accidentally publish validator signatures.
#[derive(Clone, PartialEq, Eq)]
pub struct ValidatorSignature(String);

impl ValidatorSignature {
    /// Transfers the signature into a persisted election entry.
    #[must_use]
    pub fn into_base64(self) -> String {
        self.0
    }
}

impl fmt::Debug for ValidatorSignature {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("ValidatorSignature([redacted])")
    }
}

/// Registers a key for an election interval.
///
/// The console mutation is not retried by the adapter. The workflow may repeat it
/// only after inspecting persisted election state and deciding that the same key
/// interval is still intended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AddPermanentKey {
    pub key: KeyId,
    pub election_id: u32,
    pub expire_at: u32,
}

/// Associates a temporary signing identity with a permanent validator key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AddTemporaryKey {
    pub permanent_key: KeyId,
    pub temporary_key: KeyId,
    pub expire_at: u32,
}

/// Installs an ADNL identity in the engine keyring.
///
/// Category `0` is the normal full-node/validator identity category used by
/// Localton. It remains explicit because it is part of TON's console contract,
/// not a generic networking option.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AddAdnl {
    pub key: KeyId,
    pub category: u32,
}

/// Binds consensus traffic for a validator key to a registered ADNL identity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AddValidatorAddress {
    pub validator_key: KeyId,
    pub adnl_key: KeyId,
    pub expire_at: u32,
}

/// Selects the engine's full-node ADNL identity.
///
/// Applying this transition can intentionally disconnect the active control
/// request in TON v2026.06. The official adapter recognizes that exact successful
/// disconnect, but deciding whether to restart the temporary engine remains a
/// workflow responsibility.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChangeFullNodeAddress {
    pub adnl_key: KeyId,
}

/// Imports an externally generated private validator key into the engine.
///
/// The path is passed to the official process, but file contents are never read,
/// logged, or included in adapter diagnostics. Import retry is intentionally left
/// to the bootstrap workflow because some release failures require an engine
/// restart before the same file can be accepted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportPrivateKey {
    pub private_key: PathBuf,
}

/// Requests a signature from an engine-owned key without exposing a raw hex CLI
/// contract to workflows.
///
/// Payload bytes are encoded only inside the adapter. The custom `Debug`
/// implementation and redacted command executor prevent those bytes from
/// appearing in tracing or non-zero-exit diagnostics. Signing is read-only and is
/// deterministic for a fixed key and payload.
#[derive(Clone, PartialEq, Eq)]
pub struct SignRequest {
    pub key: KeyId,
    pub payload: Vec<u8>,
}

impl fmt::Debug for SignRequest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SignRequest")
            .field("key", &self.key)
            .field("payload", &"[redacted]")
            .finish()
    }
}

/// Semantic operations supported by `validator-engine-console`.
///
/// Every mutating method performs exactly one attempt. This is crucial for key
/// creation and identity changes: blanket retries can create extra keys or hide a
/// lifecycle transition. Implementations may normalize a release-specific success
/// signal, but orchestration owns retries, ordering, readiness polling, and engine
/// restarts.
#[async_trait]
pub trait ValidatorConsole: Send + Sync {
    /// Probes the authenticated endpoint and returns parsed stats.
    ///
    /// This operation is read-only and safe for bounded workflow polling. One call
    /// performs one probe; the adapter does not loop until ready.
    async fn health(
        &self,
        context: &OperationContext,
        endpoint: &ValidatorConsoleEndpoint,
    ) -> Result<ValidatorStats>;

    /// Creates a new engine-owned key and returns its validated identifier.
    ///
    /// This operation is non-idempotent and is never retried automatically.
    async fn new_key(
        &self,
        context: &OperationContext,
        endpoint: &ValidatorConsoleEndpoint,
    ) -> Result<KeyId>;

    /// Exports a key's public half without exposing private key material.
    ///
    /// This read-only operation is safe to retry at the workflow level.
    async fn export_public(
        &self,
        context: &OperationContext,
        endpoint: &ValidatorConsoleEndpoint,
        key: &KeyId,
    ) -> Result<TonPublicKey>;

    /// Registers a permanent validator key interval exactly once.
    async fn add_permanent_key(
        &self,
        context: &OperationContext,
        endpoint: &ValidatorConsoleEndpoint,
        request: AddPermanentKey,
    ) -> Result<()>;

    /// Associates a temporary key with a permanent validator key exactly once.
    async fn add_temporary_key(
        &self,
        context: &OperationContext,
        endpoint: &ValidatorConsoleEndpoint,
        request: AddTemporaryKey,
    ) -> Result<()>;

    /// Registers one ADNL identity exactly once.
    async fn add_adnl(
        &self,
        context: &OperationContext,
        endpoint: &ValidatorConsoleEndpoint,
        request: AddAdnl,
    ) -> Result<()>;

    /// Binds validator consensus traffic to an ADNL identity exactly once.
    async fn add_validator_address(
        &self,
        context: &OperationContext,
        endpoint: &ValidatorConsoleEndpoint,
        request: AddValidatorAddress,
    ) -> Result<()>;

    /// Changes the full-node identity, accepting the pinned release's expected
    /// post-commit disconnect as success.
    async fn change_full_node_address(
        &self,
        context: &OperationContext,
        endpoint: &ValidatorConsoleEndpoint,
        request: ChangeFullNodeAddress,
    ) -> Result<()>;

    /// Imports an external private key exactly once without reading its contents
    /// in Localton.
    async fn import_private_key(
        &self,
        context: &OperationContext,
        endpoint: &ValidatorConsoleEndpoint,
        request: ImportPrivateKey,
    ) -> Result<()>;

    /// Signs bytes with an engine-owned key and validates the 64-byte result.
    async fn sign(
        &self,
        context: &OperationContext,
        endpoint: &ValidatorConsoleEndpoint,
        request: SignRequest,
    ) -> Result<ValidatorSignature>;
}

/// Production adapter for the pinned official console executable.
///
/// `connect_timeout` controls the console's internal `-t` handshake limit; the
/// outer [`OperationContext::timeout`] independently bounds the complete child
/// process. Keeping the two deadlines separate preserves useful diagnostics when
/// ADNL authentication succeeds but the remote command stalls.
#[derive(Debug, Clone)]
pub struct OfficialValidatorConsole {
    executable: PathBuf,
    connect_timeout: Duration,
}

impl OfficialValidatorConsole {
    /// Creates an adapter with the 10-second handshake timeout used by the pinned
    /// release integration.
    #[must_use]
    pub fn new(executable: impl Into<PathBuf>) -> Self {
        Self {
            executable: executable.into(),
            connect_timeout: DEFAULT_CONNECT_TIMEOUT,
        }
    }

    /// Executes one typed command and includes parsing in the semantic operation's
    /// duration and outcome.
    ///
    /// No rendered `-rc` text is included in tracing. This is especially important
    /// for `sign`, whose command contains the complete payload in hexadecimal.
    async fn execute<T>(
        &self,
        context: &OperationContext,
        endpoint: &ValidatorConsoleEndpoint,
        remote: ConsoleCommand,
        parse: impl FnOnce(&str) -> Result<T>,
    ) -> Result<T> {
        let operation = remote.operation();
        let started = Instant::now();
        trace_progress(context.node_name.as_deref(), operation, "connecting");
        let result = async {
            ensure!(
                endpoint.client_private_key.is_file(),
                "validator console client key does not exist: {}",
                endpoint.client_private_key.display()
            );
            ensure!(
                endpoint.server_public_key.is_file(),
                "validator console server key does not exist: {}",
                endpoint.server_public_key.display()
            );
            let rendered = remote.render();
            let command = self.command(endpoint, &rendered, context.timeout);
            trace_progress(context.node_name.as_deref(), operation, "executing");
            let output = if remote.contains_signature_payload() {
                run_redacted_checked(operation, command, context.timeout).await?
            } else {
                match run_checked(
                    &format!("validator-engine-console {operation}"),
                    command,
                    context.timeout,
                )
                .await
                {
                    Ok(output) => join_output(&output.stdout, &output.stderr),
                    Err(error)
                        if remote.accepts_identity_disconnect()
                            && error.to_string().contains("conn ready") =>
                    {
                        // TON v2026.06 commits `changefullnodeaddr` before replacing
                        // the active ADNL identity. validator-engine-console then
                        // exits with code 2 even though the requested state is
                        // durable. Normalize only this command and observed marker.
                        String::new()
                    }
                    Err(error) => return Err(error),
                }
            };
            parse(&output)
        }
        .await;
        trace_outcome(context.node_name.as_deref(), operation, started, &result);
        result
    }

    /// Renders the fixed transport/authentication argv shared by every semantic
    /// operation. Only this private boundary can inject a raw `-rc` string.
    fn command(
        &self,
        endpoint: &ValidatorConsoleEndpoint,
        remote_command: &str,
        operation_timeout: Duration,
    ) -> Command {
        // The child command must never keep its transport handshake alive longer
        // than the typed operation that owns it. Readiness probes can therefore
        // fail quickly without weakening the ordinary administrative commands.
        let connect_timeout = self
            .connect_timeout
            .min(operation_timeout)
            .as_secs()
            .max(1)
            .to_string();
        let mut command = Command::new(&self.executable);
        command
            .args(["-t", &connect_timeout, "-k"])
            .arg(&endpoint.client_private_key)
            .arg("-p")
            .arg(&endpoint.server_public_key)
            .args([
                "-v",
                "0",
                "-a",
                &endpoint.address.to_string(),
                "-rc",
                remote_command,
            ]);
        command
    }
}

#[async_trait]
impl ValidatorConsole for OfficialValidatorConsole {
    async fn health(
        &self,
        context: &OperationContext,
        endpoint: &ValidatorConsoleEndpoint,
    ) -> Result<ValidatorStats> {
        self.execute(context, endpoint, ConsoleCommand::Health, parse_stats)
            .await
    }

    async fn new_key(
        &self,
        context: &OperationContext,
        endpoint: &ValidatorConsoleEndpoint,
    ) -> Result<KeyId> {
        self.execute(context, endpoint, ConsoleCommand::NewKey, parse_new_key)
            .await
    }

    async fn export_public(
        &self,
        context: &OperationContext,
        endpoint: &ValidatorConsoleEndpoint,
        key: &KeyId,
    ) -> Result<TonPublicKey> {
        self.execute(
            context,
            endpoint,
            ConsoleCommand::ExportPublic { key: *key },
            parse_public_key,
        )
        .await
    }

    async fn add_permanent_key(
        &self,
        context: &OperationContext,
        endpoint: &ValidatorConsoleEndpoint,
        request: AddPermanentKey,
    ) -> Result<()> {
        self.execute(
            context,
            endpoint,
            ConsoleCommand::AddPermanentKey(request),
            ensure_console_success,
        )
        .await
    }

    async fn add_temporary_key(
        &self,
        context: &OperationContext,
        endpoint: &ValidatorConsoleEndpoint,
        request: AddTemporaryKey,
    ) -> Result<()> {
        self.execute(
            context,
            endpoint,
            ConsoleCommand::AddTemporaryKey(request),
            ensure_console_success,
        )
        .await
    }

    async fn add_adnl(
        &self,
        context: &OperationContext,
        endpoint: &ValidatorConsoleEndpoint,
        request: AddAdnl,
    ) -> Result<()> {
        self.execute(
            context,
            endpoint,
            ConsoleCommand::AddAdnl(request),
            ensure_console_success,
        )
        .await
    }

    async fn add_validator_address(
        &self,
        context: &OperationContext,
        endpoint: &ValidatorConsoleEndpoint,
        request: AddValidatorAddress,
    ) -> Result<()> {
        self.execute(
            context,
            endpoint,
            ConsoleCommand::AddValidatorAddress(request),
            ensure_console_success,
        )
        .await
    }

    async fn change_full_node_address(
        &self,
        context: &OperationContext,
        endpoint: &ValidatorConsoleEndpoint,
        request: ChangeFullNodeAddress,
    ) -> Result<()> {
        self.execute(
            context,
            endpoint,
            ConsoleCommand::ChangeFullNodeAddress(request),
            ensure_console_success,
        )
        .await
    }

    async fn import_private_key(
        &self,
        context: &OperationContext,
        endpoint: &ValidatorConsoleEndpoint,
        request: ImportPrivateKey,
    ) -> Result<()> {
        self.execute(
            context,
            endpoint,
            ConsoleCommand::ImportPrivateKey(request),
            ensure_console_success,
        )
        .await
    }

    async fn sign(
        &self,
        context: &OperationContext,
        endpoint: &ValidatorConsoleEndpoint,
        request: SignRequest,
    ) -> Result<ValidatorSignature> {
        self.execute(
            context,
            endpoint,
            ConsoleCommand::Sign(request),
            parse_signature,
        )
        .await
    }
}

/// Private command algebra for the pinned console release.
///
/// Workflows cannot construct arbitrary remote commands. This guarantees that
/// every supported mutation has explicit retry semantics, parsing, redaction, and
/// release-compatibility tests.
#[derive(Debug, Clone, PartialEq, Eq)]
enum ConsoleCommand {
    Health,
    NewKey,
    ExportPublic { key: KeyId },
    AddPermanentKey(AddPermanentKey),
    AddTemporaryKey(AddTemporaryKey),
    AddAdnl(AddAdnl),
    AddValidatorAddress(AddValidatorAddress),
    ChangeFullNodeAddress(ChangeFullNodeAddress),
    ImportPrivateKey(ImportPrivateKey),
    Sign(SignRequest),
}

impl ConsoleCommand {
    /// Stable low-cardinality operation name used in diagnostics and telemetry.
    const fn operation(&self) -> &'static str {
        match self {
            Self::Health => "health",
            Self::NewKey => "new_key",
            Self::ExportPublic { .. } => "export_public",
            Self::AddPermanentKey(_) => "add_permanent_key",
            Self::AddTemporaryKey(_) => "add_temporary_key",
            Self::AddAdnl(_) => "add_adnl",
            Self::AddValidatorAddress(_) => "add_validator_address",
            Self::ChangeFullNodeAddress(_) => "change_full_node_address",
            Self::ImportPrivateKey(_) => "import_private_key",
            Self::Sign(_) => "sign",
        }
    }

    /// Renders the exact `-rc` grammar understood by TON v2026.06.
    fn render(&self) -> String {
        match self {
            Self::Health => "getstats".to_owned(),
            Self::NewKey => "newkey".to_owned(),
            Self::ExportPublic { key } => format!("exportpub {key}"),
            Self::AddPermanentKey(request) => format!(
                "addpermkey {} {} {}",
                request.key, request.election_id, request.expire_at
            ),
            Self::AddTemporaryKey(request) => format!(
                "addtempkey {} {} {}",
                request.permanent_key, request.temporary_key, request.expire_at
            ),
            Self::AddAdnl(request) => {
                format!("addadnl {} {}", request.key, request.category)
            }
            Self::AddValidatorAddress(request) => format!(
                "addvalidatoraddr {} {} {}",
                request.validator_key, request.adnl_key, request.expire_at
            ),
            Self::ChangeFullNodeAddress(request) => {
                format!("changefullnodeaddr {}", request.adnl_key)
            }
            Self::ImportPrivateKey(request) => {
                format!("importf {}", request.private_key.display())
            }
            Self::Sign(request) => {
                format!("sign {} {}", request.key, hex::encode(&request.payload))
            }
        }
    }

    /// Restricts the release-specific disconnect exception to the single command
    /// that commits an ADNL identity before dropping the old connection.
    const fn accepts_identity_disconnect(&self) -> bool {
        matches!(self, Self::ChangeFullNodeAddress(_))
    }

    /// Selects the executor that redacts failed child output for commands carrying
    /// user payload bytes. Key identifiers are public hashes; the sign payload is
    /// not.
    const fn contains_signature_payload(&self) -> bool {
        matches!(self, Self::Sign(_))
    }
}

/// Parses a readiness response while preserving newly introduced release stats.
fn parse_stats(output: &str) -> Result<ValidatorStats> {
    let connection_ready = output.contains("conn ready");
    let values = output
        .lines()
        .filter_map(|line| {
            let line = line.trim();
            let value_start = line.find(char::is_whitespace)?;
            let (name, value) = line.split_at(value_start);
            name.bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'.'))
                .then(|| (name.to_owned(), value.trim().to_owned()))
        })
        .collect::<BTreeMap<_, _>>();
    ensure!(
        connection_ready || values.contains_key("unixtime"),
        "validator-engine-console returned stats before the connection was ready"
    );
    Ok(ValidatorStats {
        connection_ready,
        values,
    })
}

/// Extracts a leading unsigned sequence number from a descriptive engine status.
fn leading_u32(value: &str) -> Option<u32> {
    value
        .bytes()
        .take_while(|byte| byte.is_ascii_digit())
        .map(char::from)
        .collect::<String>()
        .parse()
        .ok()
}

/// Extracts a validated `(current, total)` pair from a state-part status line.
fn state_part_progress(value: &str) -> Option<(u32, u32)> {
    let (_, parts) = value.rsplit_once("(part ")?;
    let (current, total) = parts.strip_suffix(')')?.split_once(" out of ")?;
    let current = current.parse().ok()?;
    let total = total.parse().ok()?;

    (total > 0 && current <= total).then_some((current, total))
}

/// Parses the human-readable transfer line emitted by TON's state downloader.
///
/// `td::format::as_size` uses binary units and truncates each displayed value to
/// a whole unit. Converting it at this adapter boundary gives the rest of Localton
/// one stable byte-based representation without exposing release-specific text.
fn state_download_progress(value: &str) -> Option<StateDownloadProgress> {
    let (_, progress) = value.rsplit_once(" : ")?;
    let (sizes, estimates) = progress.split_once(" (")?;
    let (downloaded, total) = sizes.split_once('/')?;

    let mut estimates = estimates.strip_suffix(')')?.split(", ");
    let speed = estimates.next()?.strip_suffix("/s")?;
    estimates.next()?.strip_suffix('%')?;
    let remaining_seconds = estimates
        .next()?
        .strip_suffix("s remaining")?
        .parse()
        .ok()?;

    if estimates.next().is_some() {
        return None;
    }

    let downloaded_bytes = binary_size_bytes(downloaded)?;
    let total_bytes = binary_size_bytes(total)?;
    let bytes_per_second = binary_size_bytes(speed)?;

    (total_bytes > 0 && downloaded_bytes <= total_bytes).then_some(StateDownloadProgress {
        downloaded_bytes,
        total_bytes,
        bytes_per_second,
        remaining_seconds,
    })
}

/// Reconstructs a byte count from `td::format::as_size`'s B through GB output.
fn binary_size_bytes(value: &str) -> Option<u64> {
    [
        ("GB", 1_u64 << 30),
        ("MB", 1_u64 << 20),
        ("KB", 1_u64 << 10),
        ("B", 1),
    ]
    .into_iter()
    .find_map(|(suffix, multiplier)| {
        value
            .strip_suffix(suffix)?
            .parse::<u64>()
            .ok()?
            .checked_mul(multiplier)
    })
}

/// Extracts the last canonical 256-bit key identifier from noisy console output.
///
/// Some official builds print `created new key`, while others only include the
/// identifier among connection diagnostics. Selecting the last complete hex token
/// preserves the long-standing Localton behavior without making a release marker
/// part of the semantic contract.
fn parse_new_key(output: &str) -> Result<KeyId> {
    let expression = Regex::new(r"(?i)\b[0-9a-f]{64}\b")?;
    let value = expression
        .find_iter(output)
        .last()
        .map(|value| value.as_str())
        .context("validator-engine-console newkey returned no key id")?;
    value
        .parse()
        .context("validator-engine-console returned an invalid key id")
}

/// Extracts and validates the base64 TL public key returned by `exportpub`.
fn parse_public_key(output: &str) -> Result<TonPublicKey> {
    let value = token_after_ascii_marker(output, "got public key:")
        .context("validator-engine-console did not return a public key")?;
    let bytes = STANDARD
        .decode(value)
        .context("validator public key is not valid base64")?;

    TonPublicKey::from_tl_bytes(&bytes)
        .context("validator-engine-console returned an invalid public key")
}

/// Extracts a base64 signature and enforces the Ed25519 byte length.
fn parse_signature(output: &str) -> Result<ValidatorSignature> {
    let value = token_after_ascii_marker(output, "signature")
        .context("validator-engine-console did not return a signature")?;
    ensure!(
        STANDARD.decode(value)?.len() == 64,
        "validator signature must contain 64 bytes"
    );
    Ok(ValidatorSignature(value.to_owned()))
}

/// Rejects textual console failures that can be returned with process exit code
/// zero by some TON releases.
fn ensure_console_success(output: &str) -> Result<()> {
    let lower = output.to_ascii_lowercase();
    if lower.contains("failed") || lower.contains("error") {
        bail!("validator-engine-console reported a semantic failure")
    }
    Ok(())
}

/// Returns the first token after an ASCII marker without lowercasing the encoded
/// value itself.
fn token_after_ascii_marker<'a>(output: &'a str, marker: &str) -> Option<&'a str> {
    let lower = output.to_ascii_lowercase();
    let position = lower.find(marker)?;
    output[position + marker.len()..].split_whitespace().next()
}

/// Executes the `sign` child with redacted failure diagnostics.
///
/// The official console requires the complete payload in `-rc`; consequently the
/// generic [`run_checked`] correctly omits argv from tracing, but intentionally
/// retains child output in returned errors. A console build may echo signing data
/// there, so this focused executor preserves timeout, stdio capture, and
/// kill-on-drop behavior while redacting failure output. Successful output remains
/// in memory only long enough to parse the signature.
async fn run_redacted_checked(
    operation: &'static str,
    mut command: Command,
    max_duration: Duration,
) -> Result<String> {
    command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    let output = timeout(max_duration, command.output())
        .await
        .with_context(|| {
            format!(
                "validator-engine-console {operation} timed out after {}s",
                max_duration.as_secs()
            )
        })?
        .with_context(|| format!("failed to execute validator-engine-console {operation}"))?;
    if !output.status.success() {
        bail!(
            "validator-engine-console {operation} failed with {}; diagnostics redacted",
            output.status
        );
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    Ok(join_output(&stdout, &stderr))
}

/// Combines console channels without introducing a blank diagnostic line.
fn join_output(stdout: &str, stderr: &str) -> String {
    match (stdout.trim(), stderr.trim()) {
        ("", "") => String::new(),
        (stdout, "") => stdout.to_owned(),
        ("", stderr) => stderr.to_owned(),
        (stdout, stderr) => format!("{stdout}\n{stderr}"),
    }
}

/// Emits a low-cardinality, redacted progress event for one console operation.
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

/// Emits the semantic terminal outcome without rendered commands, key bytes,
/// payloads, public keys, or signatures.
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
            progress = if operation == "health" {
                "ready"
            } else {
                "complete"
            },
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

    fn key(hex_digit: char) -> KeyId {
        std::iter::repeat_n(hex_digit, 64)
            .collect::<String>()
            .parse()
            .unwrap()
    }

    #[test]
    fn command_algebra_matches_pinned_release_snapshot() {
        let first = key('a');
        let second = key('b');
        let commands = [
            (ConsoleCommand::Health, "getstats".to_owned()),
            (ConsoleCommand::NewKey, "newkey".to_owned()),
            (
                ConsoleCommand::ExportPublic { key: first },
                format!("exportpub {first}"),
            ),
            (
                ConsoleCommand::AddPermanentKey(AddPermanentKey {
                    key: first,
                    election_id: 100,
                    expire_at: 200,
                }),
                format!("addpermkey {first} 100 200"),
            ),
            (
                ConsoleCommand::AddTemporaryKey(AddTemporaryKey {
                    permanent_key: first,
                    temporary_key: second,
                    expire_at: 200,
                }),
                format!("addtempkey {first} {second} 200"),
            ),
            (
                ConsoleCommand::AddAdnl(AddAdnl {
                    key: second,
                    category: 0,
                }),
                format!("addadnl {second} 0"),
            ),
            (
                ConsoleCommand::AddValidatorAddress(AddValidatorAddress {
                    validator_key: first,
                    adnl_key: second,
                    expire_at: 200,
                }),
                format!("addvalidatoraddr {first} {second} 200"),
            ),
            (
                ConsoleCommand::ChangeFullNodeAddress(ChangeFullNodeAddress { adnl_key: second }),
                format!("changefullnodeaddr {second}"),
            ),
            (
                ConsoleCommand::ImportPrivateKey(ImportPrivateKey {
                    private_key: PathBuf::from("/state/keyring/validator"),
                }),
                "importf /state/keyring/validator".to_owned(),
            ),
            (
                ConsoleCommand::Sign(SignRequest {
                    key: first,
                    payload: vec![0xde, 0xad, 0xbe, 0xef],
                }),
                format!("sign {first} deadbeef"),
            ),
        ];

        for (command, expected) in commands {
            assert_eq!(command.render(), expected);
        }
    }

    #[test]
    fn console_transport_command_matches_pinned_release_snapshot() {
        let adapter = OfficialValidatorConsole::new("/ton/validator-engine-console");
        let endpoint = ValidatorConsoleEndpoint {
            address: (Ipv4Addr::LOCALHOST, 4_441).into(),
            client_private_key: PathBuf::from("/state/client"),
            server_public_key: PathBuf::from("/state/server.pub"),
        };
        let command = adapter.command(&endpoint, "getstats", Duration::from_secs(15));
        let args = command
            .as_std()
            .get_args()
            .map(OsStr::to_string_lossy)
            .map(|value| value.into_owned())
            .collect::<Vec<_>>();

        assert_eq!(
            args,
            [
                "-t",
                "10",
                "-k",
                "/state/client",
                "-p",
                "/state/server.pub",
                "-v",
                "0",
                "-a",
                "127.0.0.1:4441",
                "-rc",
                "getstats",
            ]
        );

        let readiness = adapter.command(&endpoint, "getstats", Duration::from_secs(2));
        let readiness_args = readiness
            .as_std()
            .get_args()
            .map(OsStr::to_string_lossy)
            .map(|value| value.into_owned())
            .collect::<Vec<_>>();
        assert_eq!(readiness_args[0..2], ["-t", "2"]);
    }

    #[test]
    fn parses_noisy_key_and_stats_output() {
        let expected = key('c');
        let parsed = parse_new_key(&format!("log\ncreated new key {expected}\n")).unwrap();
        assert_eq!(parsed, expected);
        let earlier = key('b');
        let parsed = parse_new_key(&format!(
            "connection identity {earlier}\nengine response {expected}\n"
        ))
        .unwrap();
        assert_eq!(parsed, expected);

        let stats = parse_stats(
            "connecting\nconn ready\nunixtime 1787985862\nmasterchainblocktime 1787985860\nprocess.initial_sync downloading masterchain state 49152000\nprocess.download_state (-1,8000000000000000,49152000) : downloading state part (part 3 out of 8)\nprocess.download_state_net (-1,8000000000000000,49152000) : 4896MB/10088MB (5968KB/s, 48.53%, 890s remaining)\n",
        )
        .unwrap();
        assert!(stats.connection_ready());
        assert_eq!(stats.unix_time().unwrap(), 1_787_985_862);
        assert_eq!(stats.masterchain_block_time().unwrap(), Some(1_787_985_860));
        expect_test::expect![[r#"
            Some(
                InitialSyncProgress {
                    stage: DownloadingMasterchainState,
                    masterchain_seqno: Some(
                        49152000,
                    ),
                    current_part: Some(
                        3,
                    ),
                    total_parts: Some(
                        8,
                    ),
                    state_download: Some(
                        StateDownloadProgress {
                            downloaded_bytes: 5133828096,
                            total_bytes: 10578034688,
                            bytes_per_second: 6111232,
                            remaining_seconds: 890,
                        },
                    ),
                },
            )
        "#]]
        .assert_debug_eq(&stats.initial_sync_progress());

        let early =
            parse_stats("unixtime 1787985862\nprocess.initial_sync starting, init block seqno 0\n")
                .unwrap();
        assert_eq!(early.masterchain_block_time().unwrap(), None);
    }

    #[test]
    fn validates_public_key_and_signature_outputs() {
        let public_key = TonPublicKey::from_bytes([7_u8; 32]).to_tl_base64();
        assert_eq!(
            parse_public_key(&format!("got public key: {public_key}"))
                .unwrap()
                .to_tl_base64(),
            public_key
        );

        let signature = STANDARD.encode([9_u8; 64]);
        assert_eq!(
            parse_signature(&format!("signature {signature}"))
                .unwrap()
                .into_base64(),
            signature
        );
        assert!(parse_signature(&format!("signature {public_key}")).is_err());
    }

    #[test]
    fn sign_debug_redacts_payload() {
        let request = SignRequest {
            key: key('d'),
            payload: vec![0xde, 0xad, 0xbe, 0xef],
        };
        let rendered = format!("{request:?}");
        assert!(rendered.contains("[redacted]"));
        assert!(!rendered.contains("deadbeef"));
    }
}
