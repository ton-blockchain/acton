//! Typed access to TON liteservers through native ADNL.
//!
//! Localton needs machine-readable protocol data, so this boundary exposes only
//! the native implementation. Human-oriented `lite-client` commands remain in
//! the CLI command that prints their output and are not a second application API.

use std::{fmt, future::Future, path::PathBuf, time::Instant};

use anyhow::{Context, Result, bail, ensure};
use async_trait::async_trait;
use base64::{Engine, engine::general_purpose::STANDARD as BASE64};
use num_bigint::BigInt;
use serde::Serialize;
use sha2::{Digest, Sha256};
use ton::{block_tlb::TVMStackValue, ton_core::traits::tlb::TLB};
use tonutils::tvm::Address;
use tracing::{Instrument, debug, field, info_span};
use tycho_types::models::config::ValidatorSet as ChainValidatorSet;

use crate::ton::lite::{AccountInfo, BlockRef, LocalLiteClient, TransactionRef};

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
}

impl LiteTarget {
    /// Creates a target backed by a trusted TON global configuration.
    pub fn new(global_config: impl Into<PathBuf>) -> Self {
        Self {
            global_config: global_config.into(),
            label: None,
        }
    }

    /// Adds a stable operator-facing identity without affecting server selection.
    pub fn with_label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// Selects one non-secret identity for structured diagnostics.
    fn diagnostic_name(&self, context: &OperationContext) -> String {
        context
            .node_name
            .clone()
            .or_else(|| self.label.clone())
            .unwrap_or_else(|| self.global_config.display().to_string())
    }
}

/// Semantic liteserver operations supported by Localton.
///
/// These names are intentionally independent of official CLI spelling such as
/// `byseqno`; diagnostics and workflow errors therefore remain stable when a
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
/// output from a transport dependency. Product workflows normally consume
/// integer entries.
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

/// On-chain validator set needed by election automation and status reporting.
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
    /// Minimum stake accepted by Elector, in nanotons.
    pub min_stake_nano: u64,
    /// Maximum stake accepted by Elector, in nanotons.
    pub max_stake_nano: u64,
    /// Network-wide stake required for a successful election, in nanotons.
    pub min_total_stake_nano: u64,
    /// Maximum effective-stake ratio encoded as a fixed-point Q16 value.
    pub max_stake_factor_q16: u32,
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

/// Semantic, object-safe liteserver boundary used by Localton workflows.
///
/// Implementations are stateless at this boundary so [`LiteClient`] can live in the
/// shared toolchain as `Arc<dyn LiteClient>`. Every operation owns its connection:
/// [`NativeLiteClient`] opens a fresh ADNL/TCP session from [`LiteTarget`]. A live
/// ADNL client must not be cached behind this trait because its continuous cipher
/// and request-correlation
/// state would require hidden serialization and ambiguous reconnect ownership.
#[async_trait]
pub trait LiteClient: Send + Sync {
    /// Reads the latest masterchain block known to the selected liteserver.
    async fn masterchain_info(
        &self,
        context: &OperationContext,
        target: &LiteTarget,
    ) -> Result<MasterchainInfo>;

    /// Reads the current account state without claiming proof verification.
    async fn account_state(
        &self,
        context: &OperationContext,
        target: &LiteTarget,
        request: AccountStateRequest,
    ) -> Result<AccountInfo>;

    /// Resolves a full block identity from workchain, shard, and seqno.
    async fn lookup_block(
        &self,
        context: &OperationContext,
        target: &LiteTarget,
        request: LookupBlock,
    ) -> Result<BlockRef>;

    /// Resolves and downloads a block while checking its serialized file hash.
    async fn block(
        &self,
        context: &OperationContext,
        target: &LiteTarget,
        request: LookupBlock,
    ) -> Result<BlockData>;

    /// Lists a bounded page of transaction identifiers from a resolved block.
    async fn block_transactions(
        &self,
        context: &OperationContext,
        target: &LiteTarget,
        request: BlockTransactionsRequest,
    ) -> Result<BlockTransactions>;

    /// Submits an external-message BoC without logging its serialized payload.
    async fn send_boc(
        &self,
        context: &OperationContext,
        target: &LiteTarget,
        message: Boc,
    ) -> Result<SendBocResult>;

    /// Executes a typed get method without accepting raw client command syntax.
    async fn run_method(
        &self,
        context: &OperationContext,
        target: &LiteTarget,
        request: RunMethodRequest,
    ) -> Result<RunMethodResult>;

    /// Decodes election timing and validator sets from the latest chain config.
    async fn election_status(
        &self,
        context: &OperationContext,
        target: &LiteTarget,
    ) -> Result<ElectionStatus>;
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
    ) -> Result<MasterchainInfo> {
        observe_native(context, target, LiteOperation::MasterchainInfo, async {
            let mut client = LocalLiteClient::connect(&target.global_config).await?;
            Ok(MasterchainInfo {
                last: client.last().await?,
            })
        })
        .await
    }

    async fn account_state(
        &self,
        context: &OperationContext,
        target: &LiteTarget,
        request: AccountStateRequest,
    ) -> Result<AccountInfo> {
        observe_native(context, target, LiteOperation::AccountState, async {
            let mut client = LocalLiteClient::connect(&target.global_config).await?;
            client.account(request.address()).await
        })
        .await
    }

    async fn lookup_block(
        &self,
        context: &OperationContext,
        target: &LiteTarget,
        request: LookupBlock,
    ) -> Result<BlockRef> {
        observe_native(context, target, LiteOperation::LookupBlock, async {
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
            Ok(id)
        })
        .await
    }

    async fn block(
        &self,
        context: &OperationContext,
        target: &LiteTarget,
        request: LookupBlock,
    ) -> Result<BlockData> {
        observe_native(context, target, LiteOperation::Block, async {
            let mut client = LocalLiteClient::connect(&target.global_config).await?;
            let (id, bytes) = client
                .block(
                    request.workchain,
                    &format_shard(request.shard),
                    request.seqno,
                )
                .await?;
            verify_file_hash(&id, &bytes)?;
            Ok(BlockData {
                id,
                boc: Boc(bytes),
            })
        })
        .await
    }

    async fn block_transactions(
        &self,
        context: &OperationContext,
        target: &LiteTarget,
        request: BlockTransactionsRequest,
    ) -> Result<BlockTransactions> {
        observe_native(context, target, LiteOperation::BlockTransactions, async {
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
            Ok(BlockTransactions {
                block,
                transactions,
                incomplete,
            })
        })
        .await
    }

    async fn send_boc(
        &self,
        context: &OperationContext,
        target: &LiteTarget,
        message: Boc,
    ) -> Result<SendBocResult> {
        let byte_len = message.len();
        observe_native(context, target, LiteOperation::SendBoc, async {
            let mut client = LocalLiteClient::connect(&target.global_config).await?;
            debug!(message.byte_len = byte_len, "submitting external message");
            Ok(SendBocResult {
                status: client.send_boc(message.into_bytes()).await?,
            })
        })
        .await
    }

    async fn run_method(
        &self,
        context: &OperationContext,
        target: &LiteTarget,
        request: RunMethodRequest,
    ) -> Result<RunMethodResult> {
        observe_native(context, target, LiteOperation::RunMethod, async {
            let mut client = LocalLiteClient::connect(&target.global_config).await?;
            let stack = client
                .run_method(
                    request.address(),
                    request.method(),
                    request.arguments().to_vec(),
                )
                .await?
                .iter()
                .map(stack_value)
                .collect::<Result<Vec<_>>>()?;
            Ok(RunMethodResult { stack })
        })
        .await
    }

    async fn election_status(
        &self,
        context: &OperationContext,
        target: &LiteTarget,
    ) -> Result<ElectionStatus> {
        observe_native(context, target, LiteOperation::ElectionStatus, async {
            let mut client = LocalLiteClient::connect(&target.global_config).await?;
            let config = client.config_params(vec![1, 15, 17, 34, 36]).await?;
            let timing = config
                .get_election_timings()
                .context("config parameter 15 has invalid election timing")?;
            let stakes = config
                .get_validator_stake_params()
                .context("config parameter 17 has invalid validator stake limits")?;
            let elector = config
                .get_elector_address()
                .context("config parameter 1 has no valid Elector address")?;
            let current = config
                .get_current_validator_set()
                .context("config parameter 34 has no valid current validator set")?;
            let next = config
                .get_next_validator_set()
                .context("config parameter 36 has an invalid next validator set")?;
            Ok(ElectionStatus {
                elector_address: Address::new(-1, elector.0).to_raw(),
                validators_elected_for: timing.validators_elected_for,
                elections_start_before: timing.elections_start_before,
                elections_end_before: timing.elections_end_before,
                stake_held_for: timing.stake_held_for,
                min_stake_nano: u64::try_from(stakes.min_stake.into_inner())
                    .context("config parameter 17 min_stake exceeds u64")?,
                max_stake_nano: u64::try_from(stakes.max_stake.into_inner())
                    .context("config parameter 17 max_stake exceeds u64")?,
                min_total_stake_nano: u64::try_from(stakes.min_total_stake.into_inner())
                    .context("config parameter 17 min_total_stake exceeds u64")?,
                max_stake_factor_q16: stakes.max_stake_factor,
                current: validator_set_info(current)?,
                next: next.map(validator_set_info).transpose()?,
            })
        })
        .await
    }
}

/// Converts canonical TVM stack values into Localton's stable response model.
fn stack_value(value: &TVMStackValue) -> Result<StackValue> {
    Ok(match value {
        TVMStackValue::Null(_) => StackValue::Null,
        TVMStackValue::TinyInt(value) => StackValue::Int {
            decimal: value.value.to_string(),
        },
        TVMStackValue::Int(value) => StackValue::Int {
            decimal: value.value.to_string(),
        },
        TVMStackValue::Cell(cell) => StackValue::Cell {
            boc_base64: BASE64.encode(cell.value.to_boc()?),
        },
        TVMStackValue::CellSlice(slice) => StackValue::Slice {
            boc_base64: BASE64.encode(slice.to_cell()?.to_boc()?),
        },
        TVMStackValue::Tuple(values) => StackValue::Tuple {
            values: values.iter().map(stack_value).collect::<Result<Vec<_>>>()?,
        },
        _ => StackValue::Unsupported {
            bytes_hex: hex::encode(value.to_boc()?),
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

/// Applies the workflow deadline to one in-process liteserver request.
///
/// The subprocess adapter enforces the same context in its process runner. Native
/// ADNL requests have no child-process boundary, so they consume the deadline here
/// and callers must not add another timeout around the semantic operation.
async fn observe_native<T>(
    context: &OperationContext,
    target: &LiteTarget,
    operation: LiteOperation,
    future: impl Future<Output = Result<T>>,
) -> Result<T> {
    let operation_name = operation.as_str();
    observe(context, target, operation, async {
        tokio::time::timeout(context.timeout, future)
            .await
            .with_context(|| {
                format!(
                    "native lite-client {operation_name} timed out after {}ms",
                    context.timeout.as_millis()
                )
            })?
    })
    .await
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
    let endpoint = "configured liteserver";
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

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use expect_test::expect;

    use super::*;

    #[test]
    fn boc_debug_output_exposes_only_size() {
        let boc = Boc::new(vec![0xde, 0xad, 0xbe, 0xef]).unwrap();
        expect!["Boc {\n    byte_len: 4,\n}\n"].assert_debug_eq(&boc);
    }

    #[test]
    fn trait_remains_object_safe() {
        fn accepts_object(_: &dyn LiteClient) {}
        let _ = accepts_object;
    }

    #[tokio::test]
    async fn native_operation_consumes_the_context_timeout_once() {
        let context = OperationContext::for_node(Duration::from_millis(1), "node2");
        let target = LiteTarget::new("global.config.json");
        let error = observe_native::<()>(
            &context,
            &target,
            LiteOperation::MasterchainInfo,
            std::future::pending(),
        )
        .await
        .unwrap_err();
        let error = format!("{error:#}");

        expect![
            "lite-client masterchain_info failed for node2 (configured liteserver): native lite-client masterchain_info timed out after 1ms: deadline has elapsed"
        ]
        .assert_eq(&error);
    }
}
