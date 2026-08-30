//! Typed access to TON liteservers through native ADNL or the official client.
//!
//! Localton uses the native implementation for machine-readable chain data. The
//! official executable remains valuable as an independent compatibility and
//! diagnostic path, but its presentation-oriented stdout is deliberately not
//! treated as a stable protocol schema.

use std::{
    fmt,
    future::Future,
    io::Write,
    os::unix::fs::OpenOptionsExt,
    path::{Path, PathBuf},
    time::Instant,
};

use anyhow::{Context, Result, bail, ensure};
use async_trait::async_trait;
use base64::{Engine, engine::general_purpose::STANDARD as BASE64};
use num_bigint::BigInt;
use rand::random;
use serde::Serialize;
use sha2::{Digest, Sha256};
use tokio::process::Command;
use tonutils::tvm::{Address, TvmStackEntry, boc::serialize_boc};
use tracing::{Instrument, debug, field, info_span};
use tycho_types::{
    boc::Boc as TychoBoc,
    models::config::{BlockchainConfigParams, ValidatorSet as ChainValidatorSet},
};

use crate::{
    binaries::TonBinaries,
    runtime::{CommandOutput, run_checked},
    ton::lite::{AccountInfo, BlockRef, LocalLiteClient, TransactionRef},
};

use super::types::{OperationContext, TonPublicKey};

/// A liteserver selection supplied by the Localton workflow.
///
/// The global configuration contains the trusted Ed25519 identity used to
/// authenticate the ADNL/TCP server. That authentication protects the transport;
/// it does not by itself verify block signatures or Merkle proofs. `endpoint` is
/// diagnostic metadata and must describe the same server selected by the config.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LiteTarget {
    /// Global TON configuration used to select and authenticate a liteserver.
    pub global_config: PathBuf,
    /// Human-readable node or network identity used in traces and errors.
    pub label: Option<String>,
    /// Selected `ip:port`, when the caller knows it without parsing the config.
    pub endpoint: Option<String>,
}

impl LiteTarget {
    /// Creates a target backed by a trusted TON global configuration.
    pub fn new(global_config: impl Into<PathBuf>) -> Self {
        Self {
            global_config: global_config.into(),
            label: None,
            endpoint: None,
        }
    }

    /// Adds a stable operator-facing identity without affecting server selection.
    pub fn with_label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// Records the selected endpoint for telemetry and actionable failures.
    ///
    /// The endpoint remains diagnostic because the trusted global configuration,
    /// rather than this string, owns the server public key and actual selection.
    pub fn with_endpoint(mut self, endpoint: impl Into<String>) -> Self {
        self.endpoint = Some(endpoint.into());
        self
    }

    /// Selects one non-secret identity for structured diagnostics.
    fn diagnostic_name(&self, context: &OperationContext) -> String {
        context
            .node_name
            .clone()
            .or_else(|| self.label.clone())
            .or_else(|| self.endpoint.clone())
            .unwrap_or_else(|| self.global_config.display().to_string())
    }
}

/// Semantic liteserver operations supported by Localton.
///
/// These names are intentionally independent of official CLI spelling such as
/// `byseqno`; observability and workflow errors therefore remain stable when a
/// pinned TON release changes its command vocabulary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LiteOperation {
    /// Read the most recent masterchain reference known to the server.
    MasterchainInfo,
    /// Read the current state of one account.
    AccountState,
    /// Resolve a block identity by workchain, shard, and sequence number.
    LookupBlock,
    /// Resolve and download a block in one operation.
    Block,
    /// Download an already resolved block identity.
    DownloadBlock,
    /// List transaction identifiers contained in one block.
    BlockTransactions,
    /// Submit an external-message bag of cells.
    SendBoc,
    /// Execute one read-only smart-contract get method.
    RunMethod,
    /// Read election timing and validator sets from on-chain configuration.
    ElectionStatus,
}

impl LiteOperation {
    /// Returns the low-cardinality operation label used by tracing backends.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::MasterchainInfo => "masterchain_info",
            Self::AccountState => "account_state",
            Self::LookupBlock => "lookup_block",
            Self::Block => "block",
            Self::DownloadBlock => "download_block",
            Self::BlockTransactions => "block_transactions",
            Self::SendBoc => "send_boc",
            Self::RunMethod => "run_method",
            Self::ElectionStatus => "election_status",
        }
    }
}

impl fmt::Display for LiteOperation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// Coordinates sufficient for the liteserver `lookupBlock` query.
///
/// The shard is stored as the signed 64-bit TON shard prefix. Rendering it as the
/// canonical 16-character hexadecimal form is owned by each transport adapter.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LookupBlock {
    /// TON workchain identifier, normally `-1` for masterchain or `0` for basechain.
    pub workchain: i32,
    /// Signed representation of the 64-bit shard prefix.
    pub shard: i64,
    /// Workchain-local block sequence number.
    pub seqno: u32,
}

/// A validated TON address used by account-state operations.
///
/// Construction normalizes friendly and raw input into raw form before it can be
/// embedded in the official client's command language. This both prevents command
/// injection and keeps native and subprocess adapters pointed at the same account.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AccountStateRequest {
    address: String,
}

impl AccountStateRequest {
    /// Parses and normalizes a friendly or raw TON address.
    pub fn new(address: &str) -> Result<Self> {
        let address = Address::from_str(address)
            .with_context(|| format!("invalid TON address `{address}`"))?
            .to_raw();
        Ok(Self { address })
    }

    /// Returns the normalized raw address accepted by both implementations.
    pub fn address(&self) -> &str {
        &self.address
    }
}

/// Typed input for one read-only smart-contract get method.
///
/// Localton currently exposes integer arguments because every product workflow
/// uses that subset. Expanding this enum later is safer than accepting arbitrary
/// lite-client stack syntax now.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RunMethodRequest {
    address: String,
    method: String,
    arguments: Vec<BigInt>,
}

impl RunMethodRequest {
    /// Validates the account and method name before either backend sees them.
    pub fn new(address: &str, method: impl Into<String>, arguments: Vec<BigInt>) -> Result<Self> {
        let address = Address::from_str(address)
            .with_context(|| format!("invalid TON address `{address}`"))?
            .to_raw();
        let method = method.into();
        ensure!(
            !method.is_empty()
                && method
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_'),
            "TON get-method name must contain only ASCII letters, digits, or underscores"
        );
        Ok(Self {
            address,
            method,
            arguments,
        })
    }

    fn address(&self) -> &str {
        &self.address
    }

    fn method(&self) -> &str {
        &self.method
    }

    fn arguments(&self) -> &[BigInt] {
        &self.arguments
    }
}

/// Stable JSON representation of values returned on a TVM stack.
///
/// Cell-like values are serialized as BoCs so the CLI never depends on Debug
/// output from `tonutils`. Product workflows normally consume integer entries.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum StackValue {
    /// TVM null.
    Null,
    /// Arbitrary-precision signed integer rendered losslessly in decimal.
    Int { decimal: String },
    /// Cell serialized as a base64 BoC.
    Cell { boc_base64: String },
    /// Slice serialized as its backing cell BoC.
    Slice { boc_base64: String },
    /// Ordered tuple values.
    Tuple { values: Vec<StackValue> },
    /// Ordered list values.
    List { values: Vec<StackValue> },
    /// Future stack constructor preserved without guessing its schema.
    Unsupported { bytes_hex: String },
}

/// Machine-readable result of a smart-contract get method.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct RunMethodResult {
    /// Values returned by TVM in stack order.
    pub stack: Vec<StackValue>,
}

impl RunMethodResult {
    /// Reads the first result as a non-negative `u64` for wallet/elector methods.
    pub fn first_u64(&self) -> Result<u64> {
        let Some(StackValue::Int { decimal }) = self.stack.first() else {
            bail!("get method did not return an integer as its first stack value");
        };
        decimal
            .parse()
            .context("get method first integer does not fit into u64")
    }
}

/// On-chain validator set needed by election automation and observability.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ValidatorSetInfo {
    /// Unix timestamp at which the set becomes active.
    pub since: u32,
    /// Unix timestamp at which the set stops being active.
    pub until: u32,
    /// Number of validators in the set.
    pub total: u16,
    /// Number of validators assigned to the masterchain subset.
    pub main: u16,
    /// Canonical lowercase Ed25519 public keys in validator order.
    pub public_keys: Vec<String>,
}

/// Election timing and current/next validator sets decoded from chain config.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ElectionStatus {
    /// Canonical raw masterchain address of the Elector contract.
    pub elector_address: String,
    /// Duration of one validator round in seconds.
    pub validators_elected_for: u32,
    /// How long before round end the entry window opens.
    pub elections_start_before: u32,
    /// How long before round end the entry window closes.
    pub elections_end_before: u32,
    /// How long elected stake remains locked after the round.
    pub stake_held_for: u32,
    /// Validator set currently securing the network.
    pub current: ValidatorSetInfo,
    /// Elected replacement set, when selection has completed.
    pub next: Option<ValidatorSetInfo>,
}

/// Parameters for a bounded block-transaction listing.
#[derive(Clone, Debug)]
pub struct BlockTransactionsRequest {
    /// Fully resolved block whose transactions should be listed.
    pub block: BlockRef,
    /// Maximum number of transaction identifiers returned by the liteserver.
    pub count: u32,
}

impl BlockTransactionsRequest {
    /// Creates a request and rejects a zero-sized page that cannot make progress.
    pub fn new(block: BlockRef, count: u32) -> Result<Self> {
        ensure!(
            count > 0,
            "block transaction count must be greater than zero"
        );
        Ok(Self { block, count })
    }
}

/// BoC bytes whose debug representation never exposes the serialized message.
///
/// External messages commonly contain signatures and application payloads. The
/// bytes are therefore available only through explicit accessors; tracing records
/// their size, never their content.
#[derive(Clone, Eq, PartialEq)]
pub struct Boc(Vec<u8>);

impl Boc {
    /// Wraps a non-empty serialized bag of cells for liteserver submission.
    pub fn new(bytes: Vec<u8>) -> Result<Self> {
        ensure!(!bytes.is_empty(), "message BoC must not be empty");
        Ok(Self(bytes))
    }

    /// Borrows serialized bytes for a transport implementation.
    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }

    /// Returns the payload size suitable for safe tracing.
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Transfers the serialized bytes into a native ADNL request.
    pub fn into_bytes(self) -> Vec<u8> {
        self.0
    }
}

impl fmt::Debug for Boc {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Boc")
            .field("byte_len", &self.len())
            .finish()
    }
}

/// Machine-readable subset of `getMasterchainInfo` currently used by Localton.
///
/// The block reference is a server response, not a proof-verification result. A
/// trustless consumer must separately verify the masterchain and response proofs.
#[derive(Clone, Debug)]
pub struct MasterchainInfo {
    /// Latest masterchain block known to the selected liteserver.
    pub last: BlockRef,
}

/// Raw block bytes paired with the identity that selected them.
///
/// The native adapter checks the serialized bytes against `file_hash`. Verifying
/// `root_hash`, block ancestry, and validator signatures belongs to a higher-level
/// proof verifier and is deliberately not implied by this type.
#[derive(Clone, Debug)]
pub struct BlockData {
    /// Resolved TON block identity.
    pub id: BlockRef,
    /// Serialized block BoC with a debug-redacted representation.
    pub boc: Boc,
}

/// A page of transaction identifiers from one block.
#[derive(Clone, Debug)]
pub struct BlockTransactions {
    /// Block returned by the liteserver, including its root and file hashes.
    pub block: BlockRef,
    /// Transaction identifiers returned in this page.
    pub transactions: Vec<TransactionRef>,
    /// Whether another page is required to observe the complete block.
    pub incomplete: bool,
}

/// Result code returned by the liteserver after submitting an external message.
///
/// Success means the liteserver accepted the request. It does not guarantee that
/// validators later included the message in a block; callers that need inclusion
/// must observe the account or transaction separately.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SendBocResult {
    /// Protocol result code returned by `liteServer.sendMessage`.
    pub status: u32,
}

/// Human-oriented result from the official `lite-client` compatibility adapter.
///
/// The executable prints C++ object renderings rather than a versioned machine
/// format. Returning this explicit variant prevents workflows from silently
/// depending on brittle regexes or confusing printed data with proof verification.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LiteDiagnostic {
    /// Semantic operation that produced the report.
    pub operation: LiteOperation,
    /// Combined stdout and stderr intended for an operator or compatibility test.
    pub output: String,
    /// Why this successful report was not converted into machine-readable data.
    pub limitation: String,
}

/// A machine-readable native result or an explicit official-client diagnostic.
///
/// `OfficialLiteClient` uses [`Self::Diagnostic`] for operations whose CLI output
/// has no stable schema. Callers that require data should keep `LocalLiteClient` as
/// the production implementation and use the official adapter for comparison.
#[derive(Clone, Debug)]
pub enum LiteResponse<T> {
    /// Structured response decoded from the TON liteserver protocol.
    Data(T),
    /// Successful official command whose output is presentation-oriented.
    Diagnostic(LiteDiagnostic),
}

impl<T> LiteResponse<T> {
    /// Extracts structured data or explains why a diagnostic backend cannot supply it.
    pub fn into_data(self) -> Result<T> {
        match self {
            Self::Data(value) => Ok(value),
            Self::Diagnostic(report) => bail!(
                "{} returned diagnostic output instead of structured data: {}",
                report.operation,
                report.limitation
            ),
        }
    }
}

/// Semantic, object-safe liteserver boundary used by Localton workflows.
///
/// Implementations are stateless at this boundary so [`LiteClient`] can live in the
/// shared toolchain as `Arc<dyn LiteClient>`. Every operation owns its connection:
/// [`NativeLiteClient`] opens a fresh ADNL/TCP session from [`LiteTarget`], while the
/// official adapter starts one bounded subprocess. A live ADNL client must not be
/// cached behind this trait because its continuous cipher and request-correlation
/// state would require hidden serialization and ambiguous reconnect ownership.
#[async_trait]
pub trait LiteClient: Send + Sync {
    /// Reads the latest masterchain block known to the selected liteserver.
    async fn masterchain_info(
        &self,
        context: &OperationContext,
        target: &LiteTarget,
    ) -> Result<LiteResponse<MasterchainInfo>>;

    /// Reads the current account state without claiming proof verification.
    async fn account_state(
        &self,
        context: &OperationContext,
        target: &LiteTarget,
        request: AccountStateRequest,
    ) -> Result<LiteResponse<AccountInfo>>;

    /// Resolves a full block identity from workchain, shard, and seqno.
    async fn lookup_block(
        &self,
        context: &OperationContext,
        target: &LiteTarget,
        request: LookupBlock,
    ) -> Result<LiteResponse<BlockRef>>;

    /// Resolves and downloads a block while checking its serialized file hash.
    async fn block(
        &self,
        context: &OperationContext,
        target: &LiteTarget,
        request: LookupBlock,
    ) -> Result<LiteResponse<BlockData>>;

    /// Downloads a resolved block and rejects a response with different hashes.
    async fn download_block(
        &self,
        context: &OperationContext,
        target: &LiteTarget,
        id: BlockRef,
    ) -> Result<LiteResponse<BlockData>>;

    /// Lists a bounded page of transaction identifiers from a resolved block.
    async fn block_transactions(
        &self,
        context: &OperationContext,
        target: &LiteTarget,
        request: BlockTransactionsRequest,
    ) -> Result<LiteResponse<BlockTransactions>>;

    /// Submits an external-message BoC without logging its serialized payload.
    async fn send_boc(
        &self,
        context: &OperationContext,
        target: &LiteTarget,
        message: Boc,
    ) -> Result<LiteResponse<SendBocResult>>;

    /// Executes a typed get method without accepting raw client command syntax.
    async fn run_method(
        &self,
        context: &OperationContext,
        target: &LiteTarget,
        request: RunMethodRequest,
    ) -> Result<LiteResponse<RunMethodResult>>;

    /// Decodes election timing and validator sets from the latest chain config.
    async fn election_status(
        &self,
        context: &OperationContext,
        target: &LiteTarget,
    ) -> Result<LiteResponse<ElectionStatus>>;
}

/// Native typed adapter that opens one authenticated ADNL/TCP session per query.
///
/// The adapter carries no connection state, so it is cheap to clone and safe to
/// share in the toolchain. Per-request connections make cancellation and target
/// ownership explicit and prevent a failed server from poisoning a long-lived
/// dependency-bundle connection.
#[derive(Clone, Copy, Debug, Default)]
pub struct NativeLiteClient;

impl NativeLiteClient {
    /// Creates the stateless native liteserver adapter.
    pub const fn new() -> Self {
        Self
    }
}

#[async_trait]
impl LiteClient for NativeLiteClient {
    async fn masterchain_info(
        &self,
        context: &OperationContext,
        target: &LiteTarget,
    ) -> Result<LiteResponse<MasterchainInfo>> {
        observe(context, target, LiteOperation::MasterchainInfo, async {
            let mut client = LocalLiteClient::connect(&target.global_config).await?;
            Ok(LiteResponse::Data(MasterchainInfo {
                last: client.last().await?,
            }))
        })
        .await
    }

    async fn account_state(
        &self,
        context: &OperationContext,
        target: &LiteTarget,
        request: AccountStateRequest,
    ) -> Result<LiteResponse<AccountInfo>> {
        observe(context, target, LiteOperation::AccountState, async {
            let mut client = LocalLiteClient::connect(&target.global_config).await?;
            Ok(LiteResponse::Data(client.account(request.address()).await?))
        })
        .await
    }

    async fn lookup_block(
        &self,
        context: &OperationContext,
        target: &LiteTarget,
        request: LookupBlock,
    ) -> Result<LiteResponse<BlockRef>> {
        observe(context, target, LiteOperation::LookupBlock, async {
            let mut client = LocalLiteClient::connect(&target.global_config).await?;
            // LocalLiteClient currently exposes lookup and download as one public
            // operation. Preserve the semantic result here and discard the checked
            // bytes until its private transport primitives move behind this trait.
            let (id, bytes) = client
                .block(
                    request.workchain,
                    &format_shard(request.shard),
                    request.seqno,
                )
                .await?;
            verify_file_hash(&id, &bytes)?;
            Ok(LiteResponse::Data(id))
        })
        .await
    }

    async fn block(
        &self,
        context: &OperationContext,
        target: &LiteTarget,
        request: LookupBlock,
    ) -> Result<LiteResponse<BlockData>> {
        observe(context, target, LiteOperation::Block, async {
            let mut client = LocalLiteClient::connect(&target.global_config).await?;
            let (id, bytes) = client
                .block(
                    request.workchain,
                    &format_shard(request.shard),
                    request.seqno,
                )
                .await?;
            verify_file_hash(&id, &bytes)?;
            Ok(LiteResponse::Data(BlockData {
                id,
                boc: Boc(bytes),
            }))
        })
        .await
    }

    async fn download_block(
        &self,
        context: &OperationContext,
        target: &LiteTarget,
        expected: BlockRef,
    ) -> Result<LiteResponse<BlockData>> {
        observe(context, target, LiteOperation::DownloadBlock, async {
            let mut client = LocalLiteClient::connect(&target.global_config).await?;
            // Re-resolve by coordinates because the native client's exact-id
            // download primitive is private today, then reject any fork mismatch.
            let (actual, bytes) = client
                .block(expected.workchain, &expected.shard, expected.seqno)
                .await?;
            ensure_same_block(&expected, &actual)?;
            verify_file_hash(&actual, &bytes)?;
            Ok(LiteResponse::Data(BlockData {
                id: actual,
                boc: Boc(bytes),
            }))
        })
        .await
    }

    async fn block_transactions(
        &self,
        context: &OperationContext,
        target: &LiteTarget,
        request: BlockTransactionsRequest,
    ) -> Result<LiteResponse<BlockTransactions>> {
        observe(context, target, LiteOperation::BlockTransactions, async {
            let mut client = LocalLiteClient::connect(&target.global_config).await?;
            let (block, transactions, incomplete) = client
                .transactions(
                    request.block.workchain,
                    &request.block.shard,
                    request.block.seqno,
                    request.count,
                )
                .await?;
            ensure_same_block(&request.block, &block)?;
            Ok(LiteResponse::Data(BlockTransactions {
                block,
                transactions,
                incomplete,
            }))
        })
        .await
    }

    async fn send_boc(
        &self,
        context: &OperationContext,
        target: &LiteTarget,
        message: Boc,
    ) -> Result<LiteResponse<SendBocResult>> {
        let byte_len = message.len();
        observe(context, target, LiteOperation::SendBoc, async {
            let mut client = LocalLiteClient::connect(&target.global_config).await?;
            debug!(message.byte_len = byte_len, "submitting external message");
            Ok(LiteResponse::Data(SendBocResult {
                status: client.send_boc(message.into_bytes()).await?,
            }))
        })
        .await
    }

    async fn run_method(
        &self,
        context: &OperationContext,
        target: &LiteTarget,
        request: RunMethodRequest,
    ) -> Result<LiteResponse<RunMethodResult>> {
        observe(context, target, LiteOperation::RunMethod, async {
            let mut client = LocalLiteClient::connect(&target.global_config).await?;
            let stack = client
                .run_method(
                    request.address(),
                    request.method(),
                    request.arguments().to_vec(),
                )
                .await?
                .into_iter()
                .map(stack_value)
                .collect::<Result<Vec<_>>>()?;
            Ok(LiteResponse::Data(RunMethodResult { stack }))
        })
        .await
    }

    async fn election_status(
        &self,
        context: &OperationContext,
        target: &LiteTarget,
    ) -> Result<LiteResponse<ElectionStatus>> {
        observe(context, target, LiteOperation::ElectionStatus, async {
            let mut client = LocalLiteClient::connect(&target.global_config).await?;
            let config = client.config_params(vec![1, 15, 34, 36]).await?;
            let config_boc = serialize_boc(&config.config, false)
                .context("failed to serialize liteserver config dictionary")?;
            let config_root = TychoBoc::decode(&config_boc)
                .context("failed to decode config dictionary with canonical TON types")?;
            let config = BlockchainConfigParams::from_raw(config_root);
            let timing = config
                .get_election_timings()
                .context("config parameter 15 has invalid election timing")?;
            let elector = config
                .get_elector_address()
                .context("config parameter 1 has no valid Elector address")?;
            let current = config
                .get_current_validator_set()
                .context("config parameter 34 has no valid current validator set")?;
            let next = config
                .get_next_validator_set()
                .context("config parameter 36 has an invalid next validator set")?;
            Ok(LiteResponse::Data(ElectionStatus {
                elector_address: Address::new(-1, elector.0).to_raw(),
                validators_elected_for: timing.validators_elected_for,
                elections_start_before: timing.elections_start_before,
                elections_end_before: timing.elections_end_before,
                stake_held_for: timing.stake_held_for,
                current: validator_set_info(current)?,
                next: next.map(validator_set_info).transpose()?,
            }))
        })
        .await
    }
}

/// Official subprocess-backed compatibility implementation of [`LiteClient`].
///
/// The executable is resolved from the same pinned [`TonBinaries`] distribution as
/// the node. Each method builds exactly one typed command and a final `quit`; no raw
/// command escape hatch is exposed. Successful human-readable output is returned as
/// [`LiteResponse::Diagnostic`] because the official CLI offers no stable JSON/TL
/// output contract for these presentation commands.
#[derive(Clone, Debug)]
pub struct OfficialLiteClient {
    binaries: TonBinaries,
}

impl OfficialLiteClient {
    /// Binds the adapter to a validated pinned TON distribution.
    pub fn new(binaries: TonBinaries) -> Self {
        Self { binaries }
    }

    /// Executes one typed official command under the workflow-owned deadline.
    ///
    /// The label contains only operation and target metadata. Neither raw argv nor
    /// BoC content is included, so timeout and process failures remain actionable
    /// without leaking signed messages into logs.
    async fn execute(
        &self,
        context: &OperationContext,
        target: &LiteTarget,
        operation: LiteOperation,
        command: OfficialLiteCommand,
    ) -> Result<LiteDiagnostic> {
        let command_texts = command.render()?;
        let target_name = target.diagnostic_name(context);
        debug!(progress.stage = "connect", progress.state = "starting");
        let mut child = Command::new(self.binaries.command("lite-client"));
        child
            .args(["-r", "-v", "0", "-L", "1024", "-t"])
            .arg(context.timeout.as_secs().max(1).to_string())
            .arg("-C")
            .arg(&target.global_config);
        for command_text in command_texts {
            child.args(["-c", &command_text]);
        }
        child.args(["-c", "quit"]);
        debug!(progress.stage = "request", progress.state = "scheduled");
        let output = run_checked(
            &format!("official lite-client {operation} for {target_name}"),
            child,
            context.timeout,
        )
        .await?;
        Ok(LiteDiagnostic {
            operation,
            output: joined_output(output),
            limitation:
                "official lite-client output is a human-readable, release-specific diagnostic"
                    .to_owned(),
        })
    }
}

#[async_trait]
impl LiteClient for OfficialLiteClient {
    async fn masterchain_info(
        &self,
        context: &OperationContext,
        target: &LiteTarget,
    ) -> Result<LiteResponse<MasterchainInfo>> {
        observe(context, target, LiteOperation::MasterchainInfo, async {
            Ok(LiteResponse::Diagnostic(
                self.execute(
                    context,
                    target,
                    LiteOperation::MasterchainInfo,
                    OfficialLiteCommand::Last,
                )
                .await?,
            ))
        })
        .await
    }

    async fn account_state(
        &self,
        context: &OperationContext,
        target: &LiteTarget,
        request: AccountStateRequest,
    ) -> Result<LiteResponse<AccountInfo>> {
        observe(context, target, LiteOperation::AccountState, async {
            Ok(LiteResponse::Diagnostic(
                self.execute(
                    context,
                    target,
                    LiteOperation::AccountState,
                    OfficialLiteCommand::AccountState(request),
                )
                .await?,
            ))
        })
        .await
    }

    async fn lookup_block(
        &self,
        context: &OperationContext,
        target: &LiteTarget,
        request: LookupBlock,
    ) -> Result<LiteResponse<BlockRef>> {
        observe(context, target, LiteOperation::LookupBlock, async {
            Ok(LiteResponse::Diagnostic(
                self.execute(
                    context,
                    target,
                    LiteOperation::LookupBlock,
                    OfficialLiteCommand::LookupBlock(request),
                )
                .await?,
            ))
        })
        .await
    }

    async fn block(
        &self,
        context: &OperationContext,
        target: &LiteTarget,
        request: LookupBlock,
    ) -> Result<LiteResponse<BlockData>> {
        observe(context, target, LiteOperation::Block, async {
            Ok(LiteResponse::Diagnostic(
                self.execute(
                    context,
                    target,
                    LiteOperation::Block,
                    OfficialLiteCommand::Block(request),
                )
                .await?,
            ))
        })
        .await
    }

    async fn download_block(
        &self,
        context: &OperationContext,
        target: &LiteTarget,
        id: BlockRef,
    ) -> Result<LiteResponse<BlockData>> {
        observe(context, target, LiteOperation::DownloadBlock, async {
            Ok(LiteResponse::Diagnostic(
                self.execute(
                    context,
                    target,
                    LiteOperation::DownloadBlock,
                    OfficialLiteCommand::DownloadBlock(id),
                )
                .await?,
            ))
        })
        .await
    }

    async fn block_transactions(
        &self,
        context: &OperationContext,
        target: &LiteTarget,
        request: BlockTransactionsRequest,
    ) -> Result<LiteResponse<BlockTransactions>> {
        observe(context, target, LiteOperation::BlockTransactions, async {
            Ok(LiteResponse::Diagnostic(
                self.execute(
                    context,
                    target,
                    LiteOperation::BlockTransactions,
                    OfficialLiteCommand::BlockTransactions(request),
                )
                .await?,
            ))
        })
        .await
    }

    async fn send_boc(
        &self,
        context: &OperationContext,
        target: &LiteTarget,
        message: Boc,
    ) -> Result<LiteResponse<SendBocResult>> {
        let byte_len = message.len();
        observe(context, target, LiteOperation::SendBoc, async {
            debug!(
                message.byte_len = byte_len,
                "staging external message for lite-client"
            );
            let staged = StagedBoc::write(message.as_bytes())?;
            let diagnostic = self
                .execute(
                    context,
                    target,
                    LiteOperation::SendBoc,
                    OfficialLiteCommand::SendFile(staged.path().to_path_buf()),
                )
                .await?;
            Ok(LiteResponse::Diagnostic(diagnostic))
        })
        .await
    }

    async fn run_method(
        &self,
        context: &OperationContext,
        target: &LiteTarget,
        request: RunMethodRequest,
    ) -> Result<LiteResponse<RunMethodResult>> {
        observe(context, target, LiteOperation::RunMethod, async {
            Ok(LiteResponse::Diagnostic(
                self.execute(
                    context,
                    target,
                    LiteOperation::RunMethod,
                    OfficialLiteCommand::RunMethod(request),
                )
                .await?,
            ))
        })
        .await
    }

    async fn election_status(
        &self,
        context: &OperationContext,
        target: &LiteTarget,
    ) -> Result<LiteResponse<ElectionStatus>> {
        observe(context, target, LiteOperation::ElectionStatus, async {
            Ok(LiteResponse::Diagnostic(
                self.execute(
                    context,
                    target,
                    LiteOperation::ElectionStatus,
                    OfficialLiteCommand::ElectionStatus,
                )
                .await?,
            ))
        })
        .await
    }
}

/// Closed set of official commands needed to implement [`LiteClient`].
///
/// Keeping this enum private makes it impossible for application workflows to
/// regain the previous arbitrary `-c <text>` escape hatch. Rendering validates all
/// values that enter the official client's own command parser.
#[derive(Debug)]
enum OfficialLiteCommand {
    Last,
    AccountState(AccountStateRequest),
    LookupBlock(LookupBlock),
    Block(LookupBlock),
    DownloadBlock(BlockRef),
    BlockTransactions(BlockTransactionsRequest),
    SendFile(PathBuf),
    RunMethod(RunMethodRequest),
    ElectionStatus,
}

impl OfficialLiteCommand {
    /// Translates semantic inputs to the pinned v2026.06 command vocabulary.
    fn render(&self) -> Result<Vec<String>> {
        let command = match self {
            Self::Last => "last".to_owned(),
            Self::AccountState(request) => format!("getaccount {}", request.address()),
            Self::LookupBlock(request) | Self::Block(request) => format!(
                "byseqno {} {} {}",
                request.workchain,
                format_shard(request.shard),
                request.seqno
            ),
            Self::DownloadBlock(id) => format!("getblock {}", format_block_id(id)?),
            Self::BlockTransactions(request) => format!(
                "listblocktrans {} {}",
                format_block_id(&request.block)?,
                request.count
            ),
            Self::SendFile(path) => format!("sendfile {}", quote_path(path)?),
            Self::RunMethod(request) => {
                let arguments = request
                    .arguments()
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(" ");
                format!(
                    "runmethod {} {} {}",
                    request.address(),
                    request.method(),
                    arguments
                )
                .trim_end()
                .to_owned()
            }
            Self::ElectionStatus => {
                return Ok(vec![
                    "getconfig 1".to_owned(),
                    "getconfig 15".to_owned(),
                    "getconfig 34".to_owned(),
                    "getconfig 36".to_owned(),
                ]);
            }
        };
        Ok(vec![command])
    }
}

/// Short-lived private file used because official `sendfile` has no stdin form.
///
/// `create_new` prevents following a pre-existing link, Unix mode `0600` avoids a
/// window where another user can read a signed message, and `Drop` removes the file
/// on success, failure, timeout, or cancellation.
struct StagedBoc {
    path: PathBuf,
}

impl StagedBoc {
    /// Writes one uniquely named BoC without ever formatting its bytes for logs.
    fn write(bytes: &[u8]) -> Result<Self> {
        for _ in 0..8 {
            let path = std::env::temp_dir().join(format!(
                "localton-lite-client-{}-{:016x}.boc",
                std::process::id(),
                random::<u64>()
            ));
            match std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(0o600)
                .open(&path)
            {
                Ok(mut file) => {
                    if let Err(error) = file.write_all(bytes) {
                        let _ = std::fs::remove_file(&path);
                        return Err(error).with_context(|| {
                            format!("failed to stage message BoC at {}", path.display())
                        });
                    }
                    return Ok(Self { path });
                }
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => {
                    return Err(error).with_context(|| {
                        format!("failed to create staged message BoC at {}", path.display())
                    });
                }
            }
        }
        bail!("failed to allocate a unique temporary file for message BoC")
    }

    /// Borrows the non-secret temporary filename passed to official `sendfile`.
    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for StagedBoc {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

/// Converts transport-library stack values into Localton's stable response model.
fn stack_value(value: TvmStackEntry) -> Result<StackValue> {
    Ok(match value {
        TvmStackEntry::Null => StackValue::Null,
        TvmStackEntry::Int(value) => StackValue::Int {
            decimal: value.to_string(),
        },
        TvmStackEntry::Cell(cell) => StackValue::Cell {
            boc_base64: BASE64.encode(serialize_boc(&cell, false)?),
        },
        TvmStackEntry::Slice(cell) => StackValue::Slice {
            boc_base64: BASE64.encode(serialize_boc(&cell, false)?),
        },
        TvmStackEntry::Tuple(values) => StackValue::Tuple {
            values: values
                .into_iter()
                .map(stack_value)
                .collect::<Result<Vec<_>>>()?,
        },
        TvmStackEntry::List(values) => StackValue::List {
            values: values
                .into_iter()
                .map(stack_value)
                .collect::<Result<Vec<_>>>()?,
        },
        TvmStackEntry::Unsupported(bytes) => StackValue::Unsupported {
            bytes_hex: hex::encode(bytes),
        },
    })
}

/// Narrows the canonical config model to fields used by election workflows.
fn validator_set_info(set: ChainValidatorSet) -> Result<ValidatorSetInfo> {
    let total = u16::try_from(set.list.len()).context("validator set exceeds u16")?;
    Ok(ValidatorSetInfo {
        since: set.utime_since,
        until: set.utime_until,
        total,
        main: set.main.get(),
        public_keys: set
            .list
            .into_iter()
            .map(|validator| TonPublicKey::from_bytes(validator.public_key.0).to_hex())
            .collect(),
    })
}

/// Runs a semantic operation inside one structured telemetry span.
///
/// `duration_ms` and `outcome` are recorded on every exit path. Progress events
/// distinguish a reused native connection from a subprocess connection attempt and
/// make a stalled connect or request visible without logging arguments or payloads.
async fn observe<T>(
    context: &OperationContext,
    target: &LiteTarget,
    operation: LiteOperation,
    future: impl Future<Output = Result<T>>,
) -> Result<T> {
    let operation_name = operation.as_str();
    let target_name = target.diagnostic_name(context);
    let endpoint = target
        .endpoint
        .as_deref()
        .unwrap_or("configured liteserver");
    let span = info_span!(
        "ton.tool.operation",
        ton.tool = "lite-client",
        operation = operation_name,
        node_or_target = %target_name,
        endpoint,
        duration_ms = field::Empty,
        outcome = field::Empty,
    );
    let recorded_span = span.clone();
    async move {
        let started = Instant::now();
        debug!(
            progress.stage = "connect",
            progress.state = "pending",
            "liteserver connection progress"
        );
        debug!(
            progress.stage = "request",
            progress.state = "started",
            "liteserver request progress"
        );
        let result = future.await;
        recorded_span.record("duration_ms", started.elapsed().as_millis() as u64);
        recorded_span.record("outcome", if result.is_ok() { "ok" } else { "error" });
        debug!(
            progress.stage = "request",
            progress.state = if result.is_ok() {
                "completed"
            } else {
                "failed"
            },
            "liteserver request progress"
        );
        result.with_context(|| {
            format!("lite-client {operation_name} failed for {target_name} ({endpoint})")
        })
    }
    .instrument(span)
    .await
}

/// Formats a TON shard prefix exactly as official lite-client expects it.
fn format_shard(shard: i64) -> String {
    format!("{:016x}", shard as u64)
}

/// Validates and formats an extended TON block identity for official commands.
fn format_block_id(id: &BlockRef) -> Result<String> {
    ensure_hex(&id.shard, 16, "block shard")?;
    ensure_hex(&id.root_hash, 64, "block root hash")?;
    ensure_hex(&id.file_hash, 64, "block file hash")?;
    Ok(format!(
        "({},{},{}):{}:{}",
        id.workchain, id.shard, id.seqno, id.root_hash, id.file_hash
    ))
}

/// Quotes a filesystem token for the official client's command parser.
fn quote_path(path: &Path) -> Result<String> {
    let value = path
        .to_str()
        .with_context(|| format!("lite-client path is not UTF-8: {}", path.display()))?;
    ensure!(
        !value.chars().any(char::is_control),
        "lite-client path contains control characters"
    );
    Ok(format!(
        "\"{}\"",
        value.replace('\\', "\\\\").replace('"', "\\\"")
    ))
}

/// Checks one fixed-size hexadecimal field before it reaches command syntax.
fn ensure_hex(value: &str, length: usize, label: &str) -> Result<()> {
    ensure!(
        value.len() == length && value.bytes().all(|byte| byte.is_ascii_hexdigit()),
        "{label} must contain exactly {length} hexadecimal characters"
    );
    Ok(())
}

/// Rejects a different fork when an exact block was requested by hash.
fn ensure_same_block(expected: &BlockRef, actual: &BlockRef) -> Result<()> {
    ensure!(
        expected.workchain == actual.workchain
            && expected.shard.eq_ignore_ascii_case(&actual.shard)
            && expected.seqno == actual.seqno
            && expected.root_hash.eq_ignore_ascii_case(&actual.root_hash)
            && expected.file_hash.eq_ignore_ascii_case(&actual.file_hash),
        "liteserver resolved a different block than requested"
    );
    Ok(())
}

/// Verifies the serialized block bytes against the advertised TON file hash.
fn verify_file_hash(id: &BlockRef, bytes: &[u8]) -> Result<()> {
    ensure_hex(&id.file_hash, 64, "block file hash")?;
    let actual = hex::encode(Sha256::digest(bytes));
    ensure!(
        id.file_hash.eq_ignore_ascii_case(&actual),
        "downloaded block file hash does not match its block id"
    );
    Ok(())
}

/// Preserves both official output streams as one operator-facing report.
fn joined_output(output: CommandOutput) -> String {
    match (output.stdout.trim(), output.stderr.trim()) {
        ("", "") => "official lite-client completed without textual output".to_owned(),
        (stdout, "") => stdout.to_owned(),
        ("", stderr) => stderr.to_owned(),
        (stdout, stderr) => format!("{stdout}\n{stderr}"),
    }
}

#[cfg(test)]
mod tests {
    use expect_test::expect;

    use super::*;

    fn block_ref() -> BlockRef {
        BlockRef {
            workchain: -1,
            shard: "8000000000000000".to_owned(),
            seqno: 42,
            root_hash: "11".repeat(32),
            file_hash: "22".repeat(32),
        }
    }

    #[test]
    fn official_commands_are_semantic_and_release_scoped() {
        let account = AccountStateRequest::new(
            "-1:3333333333333333333333333333333333333333333333333333333333333333",
        )
        .unwrap();
        let lookup = LookupBlock {
            workchain: 0,
            shard: i64::MIN,
            seqno: 42,
        };
        let commands = [
            OfficialLiteCommand::Last.render().unwrap(),
            OfficialLiteCommand::AccountState(account).render().unwrap(),
            OfficialLiteCommand::LookupBlock(lookup).render().unwrap(),
            OfficialLiteCommand::Block(lookup).render().unwrap(),
            OfficialLiteCommand::DownloadBlock(block_ref())
                .render()
                .unwrap(),
            OfficialLiteCommand::BlockTransactions(
                BlockTransactionsRequest::new(block_ref(), 100).unwrap(),
            )
            .render()
            .unwrap(),
            OfficialLiteCommand::SendFile(PathBuf::from("/tmp/message with spaces.boc"))
                .render()
                .unwrap(),
        ]
        .concat()
        .join("\n");

        expect![[r#"
            last
            getaccount -1:3333333333333333333333333333333333333333333333333333333333333333
            byseqno 0 8000000000000000 42
            byseqno 0 8000000000000000 42
            getblock (-1,8000000000000000,42):1111111111111111111111111111111111111111111111111111111111111111:2222222222222222222222222222222222222222222222222222222222222222
            listblocktrans (-1,8000000000000000,42):1111111111111111111111111111111111111111111111111111111111111111:2222222222222222222222222222222222222222222222222222222222222222 100
            sendfile "/tmp/message with spaces.boc""#]]
        .assert_eq(&commands);
    }

    #[test]
    fn boc_debug_output_exposes_only_size() {
        let boc = Boc::new(vec![0xde, 0xad, 0xbe, 0xef]).unwrap();
        expect!["Boc {\n    byte_len: 4,\n}\n"].assert_debug_eq(&boc);
    }

    #[test]
    fn official_block_ids_reject_command_language_injection() {
        let mut id = block_ref();
        id.root_hash = format!("{} quit", "11".repeat(32));
        let error = OfficialLiteCommand::DownloadBlock(id)
            .render()
            .unwrap_err()
            .to_string();
        expect![[r#"block root hash must contain exactly 64 hexadecimal characters"#]]
            .assert_eq(&error);
    }

    #[test]
    fn trait_remains_object_safe() {
        fn accepts_object(_: &dyn LiteClient) {}
        let _ = accepts_object;
    }
}
