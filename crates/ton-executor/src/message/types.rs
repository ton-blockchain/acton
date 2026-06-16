use anyhow::Context;
use num_bigint::{BigInt, Sign};
use rustc_hash::FxHashSet;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tvm_ffi::stack::{Tuple, TupleItem};
use tycho_types::boc::Boc;
use tycho_types::cell::CellBuilder;

/// Result of a transaction emulation.
#[derive(Deserialize, Debug, Clone)]
#[serde(untagged)]
pub enum EmulationResult {
    Success(RunTransactionResultSuccess),
    Error(RunTransactionResultError),
}

#[derive(Deserialize, Debug, Clone)]
pub struct RunTransactionResultSuccess {
    /// Base64 encoded transaction `BoC`.
    pub transaction: Arc<str>,
    /// Base64 encoded updated shard account `BoC`.
    pub shard_account: Arc<str>,
    /// Virtual Machine execution logs.
    pub vm_log: Arc<str>,
    /// Base64 encoded actions `BoC` (if any).
    pub actions: Option<Arc<str>>,
    /// Hashes of missing libraries observed during this emulator run.
    #[serde(default)]
    pub missing_libraries: FxHashSet<String>,
}

#[derive(Deserialize, Clone, Debug)]
pub struct RunTransactionResultError {
    /// Error message.
    pub error: String,
    /// Whether the external-in message was rejected before acceptance.
    #[serde(default)]
    pub external_not_accepted: bool,
    /// Virtual Machine execution logs (if available).
    pub vm_log: Option<String>,
    /// VM exit code (if available).
    pub vm_exit_code: Option<i64>,
    /// Set by executor.
    pub executor_logs: Option<Arc<str>>,
    /// Hashes of missing libraries observed during this emulator run.
    #[serde(default)]
    pub missing_libraries: FxHashSet<String>,
}

/// Information about previous blocks.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct PrevBlockId {
    /// Workchain ID.
    pub workchain: i32,
    /// Shard ID in signed representation used by TON APIs.
    ///
    /// It is serialized into TVM c7 as an unsigned 64-bit integer, matching TON
    /// core behavior for `BlockId`.
    pub shard: i64,
    /// Block sequence number.
    pub seqno: u32,
    /// Root hash (32 bytes).
    pub root_hash: [u8; 32],
    /// File hash (32 bytes).
    pub file_hash: [u8; 32],
}

/// Information required for `PREVBLOCKS*` TVM instructions.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct PrevBlocksInfo {
    /// List used by `PREVMCBLOCKS`.
    pub last_mc_blocks: Vec<PrevBlockId>,
    /// Block used by `PREVKEYBLOCK`.
    pub prev_key_block: PrevBlockId,
    /// List used by `PREVMCBLOCKS_100`.
    pub last_mc_blocks_100: Vec<PrevBlockId>,
}

impl PrevBlocksInfo {
    /// Creates c7 previous-blocks info for `PREVBLOCKS*` instructions.
    #[must_use]
    pub const fn new(
        last_mc_blocks: Vec<PrevBlockId>,
        prev_key_block: PrevBlockId,
        last_mc_blocks_100: Vec<PrevBlockId>,
    ) -> Self {
        Self {
            last_mc_blocks,
            prev_key_block,
            last_mc_blocks_100,
        }
    }

    /// Serializes previous-blocks info into the base64 stack-entry `BoC` expected by
    /// TON native emulators.
    pub fn to_stack_entry_boc_base64(&self) -> anyhow::Result<String> {
        let fields = vec![
            block_ids_to_tuple_item(&self.last_mc_blocks),
            block_id_to_tuple(&self.prev_key_block),
            block_ids_to_tuple_item(&self.last_mc_blocks_100),
        ];

        let tuple_item = TupleItem::Tuple(Tuple(fields));
        let mut builder = CellBuilder::new();

        tvm_ffi::serde::serialize_tuple_item(&mut builder, &tuple_item)
            .context("failed to serialize prev_blocks_info tuple item")?;

        let cell = builder
            .build()
            .context("failed to build prev_blocks_info stack-entry cell")?;

        Ok(Boc::encode_base64(&cell))
    }
}

/// Arguments for running a transaction emulation.
#[derive(Debug, Clone)]
pub struct RunTransactionArgs {
    /// Base64 encoded libraries `BoC`.
    pub libs: Option<String>,
    /// Base64 encoded shard account `BoC`.
    pub shard_account: String,
    /// Current unix time.
    pub now: u32,
    /// Current logical time.
    pub lt: u64,
    /// Optional random seed for the VM.
    pub random_seed: Option<[u8; 32]>,
    /// Whether to ignore signature checks.
    pub ignore_chksig: bool,
    /// Whether to enable debug logs.
    pub debug_enabled: bool,
    /// Information about previous blocks.
    pub prev_blocks_info: Option<PrevBlocksInfo>,
    /// Whether this is a tick-tock transaction.
    pub is_tick_tock: Option<bool>,
    /// Whether this is a "tock" transaction (if `is_tick_tock` is true).
    pub is_tock: Option<bool>,
}

impl Default for RunTransactionArgs {
    fn default() -> Self {
        Self {
            libs: None,
            shard_account: String::new(),
            now: 0,
            lt: 0,
            random_seed: None,
            ignore_chksig: false,
            debug_enabled: true,
            prev_blocks_info: None,
            is_tick_tock: None,
            is_tock: None,
        }
    }
}

#[derive(Deserialize)]
#[serde(untagged)]
pub(crate) enum EmulationInternalResult {
    Success {
        output: EmulationResult,
        logs: Arc<str>,
    },
    Fail {
        #[allow(dead_code)] // used only for decoding
        fail: bool,
        message: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct EmulationInternalParams {
    pub utime: u32,
    pub lt: String, // For some reason this field is a String in C++ code treated as u64
    pub rand_seed: String,
    pub ignore_chksig: bool,
    pub debug_enabled: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_tick_tock: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_tock: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prev_blocks_info: Option<String>,
}

impl TryFrom<&RunTransactionArgs> for EmulationInternalParams {
    type Error = anyhow::Error;

    fn try_from(args: &RunTransactionArgs) -> Result<Self, Self::Error> {
        let rand_seed = match &args.random_seed {
            Some(seed) => hex::encode(seed),
            None => String::new(),
        };

        let prev_blocks_info = args
            .prev_blocks_info
            .as_ref()
            .map(PrevBlocksInfo::to_stack_entry_boc_base64)
            .transpose()?;

        Ok(Self {
            utime: args.now,
            lt: args.lt.to_string(),
            rand_seed,
            ignore_chksig: args.ignore_chksig,
            debug_enabled: args.debug_enabled,
            is_tick_tock: args.is_tick_tock,
            is_tock: args.is_tock,
            prev_blocks_info,
        })
    }
}

fn block_id_to_tuple(block_id: &PrevBlockId) -> TupleItem {
    TupleItem::Tuple(Tuple(vec![
        TupleItem::Int(BigInt::from(block_id.workchain)),
        TupleItem::Int(BigInt::from(block_id.shard as u64)),
        TupleItem::Int(BigInt::from(block_id.seqno)),
        TupleItem::Int(BigInt::from_bytes_be(Sign::Plus, &block_id.root_hash)),
        TupleItem::Int(BigInt::from_bytes_be(Sign::Plus, &block_id.file_hash)),
    ]))
}

fn block_ids_to_tuple_item(blocks: &[PrevBlockId]) -> TupleItem {
    TupleItem::Tuple(Tuple(blocks.iter().map(block_id_to_tuple).collect()))
}
